# process-memory
A simple tool written in rust to read the memory of a process on linux.

## Installation
Build this tool with `cargo build --release`.

## Usage
Just run 
```
process-memory PID [OUTPUT_DIR]
```

`memory` will be used as output directory by default. If the output directory doesn't exist it will be created automaticaly. This tools creates a file inside of the directory for each memory page of the process.