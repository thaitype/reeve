let who = exec("whoami", []);
let host = exec("hostname", ["-s"]);
let kern = exec("uname", ["-a"]);
let dt = exec("date", ["-I"]);

// In Rhai, trim() mutates in-place (returns ())
let user_str = who.stdout;
user_str.trim();
let host_str = host.stdout;
host_str.trim();
let kern_str = kern.stdout;
kern_str.trim();
let date_str = dt.stdout;
date_str.trim();

print("=== sysinfo ===");
print(`user:     ${user_str}`);
print(`host:     ${host_str}`);
print(`kernel:   ${kern_str}`);
print(`date:     ${date_str}`);
