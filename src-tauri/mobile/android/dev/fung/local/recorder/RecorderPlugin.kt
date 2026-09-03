// Native chunked audio recorder for the FUNG mobile shell.
//
// This file is the TRACKED source of the Kotlin half of the "mobile-recorder"
// Tauri plugin. The generated Android project (src-tauri/gen/android) is
// gitignored, so scripts/mobile_android.ps1 copies this file into
//   gen/android/app/src/main/java/dev/fung/local/recorder/RecorderPlugin.kt
// on every init and build. Losing the generated tree must never lose this
// class again — the Rust side hard-requires it at startup:
//   native_recorder.rs: register_android_plugin("dev.fung.local.recorder", "RecorderPlugin")
//
// Contract with src-tauri/src/native_recorder.rs and mobile.rs:
// - commands: start / status / pause / resume / stop, args {recordingId}
// - response: {available, recordingId, state, safeOffsetMs, segmentCount,
//   segments: [{sequence, relativePath, durationMs, byteSize, checksum}]}
// - sealed segment files live in <filesDir>/native-recordings/<recordingId>/
//   (filesDir matches Tauri's app_data_dir on Android, which mobile.rs joins
//   with "native-recordings"); only the file NAME of relativePath is used.
// - checksum is lowercase hex sha256 of the file bytes; sequence starts at 1
//   and is contiguous; durationMs and byteSize must be positive.
// - frontend (MobileApp.tsx) branches on state "recording" and "paused".
package dev.fung.local.recorder

import android.Manifest
import android.app.Activity
import android.media.MediaMetadataRetriever
import android.media.MediaRecorder
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.PermissionState
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.security.MessageDigest

@InvokeArg
class RecordingArgs {
    var recordingId: String = ""
}

private data class SealedSegment(
    val sequence: Long,
    val relativePath: String,
    val durationMs: Long,
    val byteSize: Long,
    val checksum: String,
)

@TauriPlugin(
    permissions = [
        Permission(strings = [Manifest.permission.RECORD_AUDIO], alias = "microphone")
    ]
)
class RecorderPlugin(private val activity: Activity) : Plugin(activity) {
    companion object {
        // Segment length mirrors the 5s chunk cadence the web capture path used.
        private const val SEGMENT_MS = 5000
    }

    private val lock = Any()
    private var recorder: MediaRecorder? = null
    private var recordingId: String? = null
    private var recordingDir: File? = null
    private var state: String = "idle"
    private var sealed = mutableListOf<SealedSegment>()
    // File currently being written and the one queued via setNextOutputFile.
    private var currentFile: File? = null
    private var currentSequence: Long = 0
    private var nextFile: File? = null

    // ---- commands -------------------------------------------------------

    @Command
    fun start(invoke: Invoke) {
        if (getPermissionState("microphone") != PermissionState.GRANTED) {
            requestPermissionForAlias("microphone", invoke, "onMicrophonePermission")
            return
        }
        doStart(invoke)
    }

    @PermissionCallback
    fun onMicrophonePermission(invoke: Invoke) {
        if (getPermissionState("microphone") != PermissionState.GRANTED) {
            invoke.reject("microphone permission denied")
            return
        }
        doStart(invoke)
    }

    @Command
    fun status(invoke: Invoke) {
        val args = invoke.parseArgs(RecordingArgs::class.java)
        synchronized(lock) { invoke.resolve(snapshot(args.recordingId)) }
    }

    @Command
    fun pause(invoke: Invoke) {
        val args = invoke.parseArgs(RecordingArgs::class.java)
        synchronized(lock) {
            val active = recorder
            if (active == null || recordingId != args.recordingId || state != "recording") {
                invoke.resolve(snapshot(args.recordingId))
                return
            }
            try {
                active.pause()
                state = "paused"
                invoke.resolve(snapshot(args.recordingId))
            } catch (error: Exception) {
                invoke.reject("pause failed: ${error.message}")
            }
        }
    }

    @Command
    fun resume(invoke: Invoke) {
        val args = invoke.parseArgs(RecordingArgs::class.java)
        synchronized(lock) {
            val active = recorder
            if (active == null || recordingId != args.recordingId || state != "paused") {
                invoke.resolve(snapshot(args.recordingId))
                return
            }
            try {
                active.resume()
                state = "recording"
                invoke.resolve(snapshot(args.recordingId))
            } catch (error: Exception) {
                invoke.reject("resume failed: ${error.message}")
            }
        }
    }

    @Command
    fun stop(invoke: Invoke) {
        val args = invoke.parseArgs(RecordingArgs::class.java)
        synchronized(lock) {
            val active = recorder
            if (active == null || recordingId != args.recordingId) {
                invoke.resolve(snapshot(args.recordingId))
                return
            }
            try {
                try {
                    active.stop()
                    // stop() finalizes the file currently being written.
                    currentFile?.let { seal(it, currentSequence) }
                } catch (_: RuntimeException) {
                    // stop() throws if nothing valid was captured for the
                    // current file; the sealed list is still authoritative.
                }
                active.release()
            } finally {
                recorder = null
                currentFile = null
                nextFile = null
                state = "stopped"
            }
            invoke.resolve(snapshot(args.recordingId))
        }
    }

    // ---- recording internals -------------------------------------------

    private fun doStart(invoke: Invoke) {
        val args = invoke.parseArgs(RecordingArgs::class.java)
        if (args.recordingId.isBlank()) {
            invoke.reject("recordingId must not be empty")
            return
        }
        synchronized(lock) {
            if (recorder != null) {
                if (recordingId == args.recordingId) {
                    // Idempotent start for the active session.
                    invoke.resolve(snapshot(args.recordingId))
                } else {
                    invoke.reject("another recording is already active")
                }
                return
            }
            val dir = File(File(activity.filesDir, "native-recordings"), args.recordingId)
            if (!dir.isDirectory && !dir.mkdirs()) {
                invoke.reject("cannot create recording directory")
                return
            }
            recordingId = args.recordingId
            recordingDir = dir
            sealed = mutableListOf()
            currentSequence = 1
            val first = segmentFile(1)
            currentFile = first
            nextFile = null
            try {
                @Suppress("DEPRECATION")
                val mediaRecorder = MediaRecorder()
                mediaRecorder.setAudioSource(MediaRecorder.AudioSource.MIC)
                mediaRecorder.setOutputFormat(MediaRecorder.OutputFormat.MPEG_4)
                mediaRecorder.setAudioEncoder(MediaRecorder.AudioEncoder.AAC)
                mediaRecorder.setAudioSamplingRate(44100)
                mediaRecorder.setAudioEncodingBitRate(128_000)
                mediaRecorder.setMaxDuration(SEGMENT_MS)
                mediaRecorder.setOutputFile(first.absolutePath)
                mediaRecorder.setOnInfoListener { active, what, _ ->
                    onRecorderInfo(active, what)
                }
                mediaRecorder.prepare()
                mediaRecorder.start()
                recorder = mediaRecorder
                state = "recording"
                invoke.resolve(snapshot(args.recordingId))
            } catch (error: Exception) {
                recorder?.release()
                recorder = null
                currentFile = null
                state = "idle"
                invoke.reject("start failed: ${error.message}")
            }
        }
    }

    private fun onRecorderInfo(active: MediaRecorder, what: Int) {
        synchronized(lock) {
            if (active !== recorder) return
            when (what) {
                MediaRecorder.MEDIA_RECORDER_INFO_MAX_DURATION_REACHED -> {
                    // Queue the next segment; recording continues gaplessly
                    // (setNextOutputFile exists since API 26 == our minSdk).
                    try {
                        val upcoming = segmentFile(currentSequence + 1)
                        nextFile = upcoming
                        active.setNextOutputFile(upcoming)
                    } catch (_: Exception) {
                        // If rotation fails the current file keeps recording
                        // until stop(); no data is lost, chunking just stops.
                        nextFile = null
                    }
                }
                MediaRecorder.MEDIA_RECORDER_INFO_NEXT_OUTPUT_FILE_STARTED -> {
                    // The previous file is finalized now — seal it.
                    val finished = currentFile
                    val queued = nextFile
                    if (finished != null) seal(finished, currentSequence)
                    if (queued != null) {
                        currentSequence += 1
                        currentFile = queued
                        nextFile = null
                    }
                }
            }
        }
    }

    private fun segmentFile(sequence: Long): File {
        val dir = recordingDir ?: throw IllegalStateException("no recording directory")
        return File(dir, "segment-%06d.m4a".format(sequence))
    }

    private fun seal(file: File, sequence: Long) {
        if (!file.isFile || sealed.any { it.sequence == sequence }) return
        val byteSize = file.length()
        if (byteSize <= 0) return
        val durationMs = probeDurationMs(file)
        if (durationMs <= 0) return
        sealed.add(
            SealedSegment(
                sequence = sequence,
                relativePath = file.name,
                durationMs = durationMs,
                byteSize = byteSize,
                checksum = sha256Hex(file),
            )
        )
    }

    private fun probeDurationMs(file: File): Long {
        return try {
            val retriever = MediaMetadataRetriever()
            try {
                retriever.setDataSource(file.absolutePath)
                retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION)
                    ?.toLongOrNull() ?: SEGMENT_MS.toLong()
            } finally {
                retriever.release()
            }
        } catch (_: Exception) {
            SEGMENT_MS.toLong()
        }
    }

    private fun sha256Hex(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { stream ->
            val buffer = ByteArray(64 * 1024)
            while (true) {
                val read = stream.read(buffer)
                if (read < 0) break
                digest.update(buffer, 0, read)
            }
        }
        return digest.digest().joinToString("") { "%02x".format(it) }
    }

    private fun snapshot(requestedId: String): JSObject {
        val result = JSObject()
        result.put("available", true)
        val activeId = recordingId
        if (activeId == null || activeId != requestedId) {
            result.put("recordingId", requestedId)
            result.put("state", "idle")
            result.put("safeOffsetMs", 0L)
            result.put("segmentCount", 0L)
            result.put("segments", JSArray())
            return result
        }
        result.put("recordingId", activeId)
        result.put("state", state)
        result.put("safeOffsetMs", sealed.sumOf { it.durationMs })
        result.put("segmentCount", sealed.size.toLong())
        val segments = JSArray()
        for (segment in sealed.sortedBy { it.sequence }) {
            val entry = JSObject()
            entry.put("sequence", segment.sequence)
            entry.put("relativePath", segment.relativePath)
            entry.put("durationMs", segment.durationMs)
            entry.put("byteSize", segment.byteSize)
            entry.put("checksum", segment.checksum)
            segments.put(entry)
        }
        result.put("segments", segments)
        return result
    }
}
