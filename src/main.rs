use std::{
    collections::HashMap,
    env,
    fs::{create_dir, read_to_string, File},
    hash::Hash,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
    process::ExitCode,
};

type Mode = u8;
const MODE_EXEC: u8 = 1;
const MODE_WRITE: u8 = 2;
const MODE_READ: u8 = 4;

/// group a Vec<V> by keys given by fn(key: &V) -> K to HashMap<K, Vec<V>>
fn group_by<K: Hash + Eq, V>(list: Vec<V>, key_fn: fn(key: &V) -> K) -> HashMap<K, Vec<V>> {
    let mut map: HashMap<K, Vec<V>> = HashMap::new();
    for element in list {
        let key = key_fn(&element);
        if let Some(value) = map.get_mut(&key) {
            value.push(element);
        } else {
            map.insert(key, vec![element]);
        }
    }
    map
}

/// copy exact `n` bytes from `from` to `to` chunkwise
fn ncopy<R: Read, W: Write>(mut from: R, mut to: W, n: usize) {
    let mut already_read = 0;
    let mut buf = vec![0; 256];
    while already_read < n {
        if n - already_read < 256 {
            // don't read full 256 bytes, read n - already_read instead
            buf = vec![0; n - already_read];
        }
        let read = from.read(&mut buf).expect("Error while reading");
        already_read += read;
        to.write_all(&buf).expect("Error while writing");
    }
    assert_eq!(already_read, n);
}

struct VirtMemoryPage {
    from: u64, // page starts at this address
    to: u64,   // page ends at this address
    mode: Mode,
    file_path: String, // path to file, '[heap]', '[stack]', ... or emtpy
}

impl VirtMemoryPage {
    fn from_line(line: &str) -> Self {
        let parts = line.split_whitespace().collect::<Vec<&str>>();
        if parts.len() < 5 {
            panic!("Invalid virt memory part: {line}");
        }
        let mut mode = 0;
        for char in parts[1].chars() {
            match char {
                'r' => mode |= MODE_READ,
                'w' => mode |= MODE_WRITE,
                'x' => mode |= MODE_EXEC,
                'p' => (), // private, not relevant
                's' => (), // shared, not relevant
                '-' => (),
                _ => panic!("Invalid virt memory part permission character"),
            }
        }

        let splitted_range = parts[0].split('-').collect::<Vec<&str>>();
        assert_eq!(splitted_range.len(), 2);
        Self {
            from: u64::from_str_radix(splitted_range[0], 16)
                .expect("Error while parsing virt memory part range from"),
            to: u64::from_str_radix(splitted_range[1], 16)
                .expect("Error while parsing virt memory part range from"),
            mode,
            file_path: if parts.len() > 5 {
                parts[5..].join(" ")
            } else {
                String::new()
            },
        }
    }
}

fn main() -> ExitCode {
    let args = env::args().collect::<Vec<String>>();
    if args.len() < 2 {
        println!("Usage: process_memory PID [OUTPUT_DIR]");
        return ExitCode::FAILURE;
    }

    let pid = &args[1];
    let output_dir = PathBuf::from(
        args.get(2)
            .map(|s| s.to_string())
            .unwrap_or_else(|| "memory".to_string()),
    );

    if !output_dir.exists() {
        create_dir(&output_dir).expect("Error while creating output directory");
    }

    let pid_path = PathBuf::from(format!("/proc/{pid}"));
    if !pid_path.exists() {
        println!("Process with PID {pid} does not exist.");
        return ExitCode::FAILURE;
    }

    let maps_path = pid_path.join("maps");
    let maps = read_to_string(maps_path).expect("Error while reading process memory maps");
    let memory_parts = maps
        .split('\n')
        .filter(|l| !l.is_empty()) // empty lines should not be considered
        .map(VirtMemoryPage::from_line)
        .filter(|m| {
            m.mode & MODE_READ != 0 && m.mode & MODE_WRITE != 0 // memory pages that are not readable or writeable are not relevant
        })
        .collect::<Vec<VirtMemoryPage>>();
    let grouped = group_by(memory_parts, |v| v.file_path.clone());

    let mut pmemory = File::open(pid_path.join("mem")).expect("Error while opening process memory");

    for (file_path, memory_parts) in grouped {
        let dir = if memory_parts.len() > 1 {
            output_dir.clone().join(if file_path.is_empty() {
                "no-name"
            } else {
                &file_path
            })
        } else {
            output_dir.clone()
        };
        if !dir.exists() {
            create_dir(&dir).expect("Error while creating a output subdirectory");
        }

        for part in &memory_parts {
            let path = dir.clone().join(if memory_parts.len() > 1 {
                format!("{}-{}", part.from, part.to)
            } else {
                file_path.clone().replace('/', "_")
            });
            let target_file = File::create(path).expect("Error while creating memory file");
            println!("read {}", part.file_path);
            pmemory
                .seek(SeekFrom::Start(part.from))
                .expect("Error while seeking process memory");
            ncopy(&pmemory, target_file, (part.to - part.from) as usize);
        }
    }

    ExitCode::SUCCESS
}
