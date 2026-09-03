// Device-capability probe for the FUNG mobile shell's on-device AI tiering.
//
// TRACKED source of the Kotlin half of the "on-device-ai-profile" Tauri
// plugin; scripts/mobile_android.ps1 copies the whole src-tauri/mobile/android
// tree into gen/android/app/src/main/java on every init and build. The Rust
// side hard-requires this class at startup:
//   on_device_ai.rs: register_android_plugin("dev.fung.local.aiprofile", "AiProfilePlugin")
//
// Contract (on_device_ai.rs DeviceProfile, serde camelCase): the single
// command `profile` takes no meaningful args and returns
//   {sdkInt, arm64, totalRamMb, availableRamMb, freeStorageMb}.
package dev.fung.local.aiprofile

import android.app.Activity
import android.app.ActivityManager
import android.content.Context
import android.os.Build
import android.os.StatFs
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@TauriPlugin
class AiProfilePlugin(private val activity: Activity) : Plugin(activity) {
    @Command
    fun profile(invoke: Invoke) {
        val memory = ActivityManager.MemoryInfo()
        val manager = activity.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
        manager.getMemoryInfo(memory)
        val stat = StatFs(activity.filesDir.absolutePath)

        val result = JSObject()
        result.put("sdkInt", Build.VERSION.SDK_INT.toLong())
        result.put("arm64", Build.SUPPORTED_64_BIT_ABIS.contains("arm64-v8a"))
        result.put("totalRamMb", memory.totalMem / (1024L * 1024L))
        result.put("availableRamMb", memory.availMem / (1024L * 1024L))
        result.put("freeStorageMb", stat.availableBytes / (1024L * 1024L))
        invoke.resolve(result)
    }
}
