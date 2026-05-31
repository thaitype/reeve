// workspace-demo.rhai — demonstrates Layer 1 FS host functions.
// Files are written to <reeve_home>/workspace/ (sandboxed).
// write_file throws FileAlreadyExists if the file exists; use exists() first.

let filename = "demo-output.txt";

if exists(filename) {
    log_warn("demo-output.txt already exists — skipping write_file");
} else {
    write_file(filename, "Hello from workspace-demo!");
}

append_file(filename, "\nAppended line.");

let contents = read_file(filename);
print(contents);

let file_present = exists(filename);
print(`exists("demo-output.txt") = ${file_present}`);

log_info("workspace-demo complete");
