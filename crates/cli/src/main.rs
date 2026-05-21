// Foliom CLI entry point.
//
// Plan 01-01 ships an intentional stub: any invocation prints a stub
// message to stderr and exits with code 2. Plan 01-07 wires the real
// subcommands (`index`, `reindex`, `search`, `dump-tree`, `inventory`)
// via `clap`.

fn main() {
    eprintln!("foliom: not yet implemented");
    std::process::exit(2);
}
