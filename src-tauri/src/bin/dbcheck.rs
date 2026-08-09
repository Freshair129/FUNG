//! Temporary diagnostic: opens a COPY of a Genesis db, inspects rows whose
//! Json-typed columns might hold strings, then attempts the schema install.
//! Delete this file once the v8 migration issue is resolved.

fn main() {
    let path = std::env::args().nth(1).expect("usage: dbcheck <genesisdb-copy-path>");
    match fung_lib::__debug_db_probe(&path) {
        Ok(report) => println!("{report}"),
        Err(error) => println!("PROBE ERROR: {error}"),
    }
}
