use crate::println;

use crate::file_system::block::*;
use crate::file_system::dir::*;
use crate::file_system::inode::*;

const INODE_START: usize = 1;
const INODE_COUNT: usize = 16;
const DATA_START: usize = INODE_START + INODE_COUNT;

pub struct FileSystem {
    pub disk: Disk,
    next_data_block: usize,
}
use lazy_static::lazy_static;
use spin::Mutex;

pub static FS: Mutex<FileSystem> = Mutex::new(FileSystem {
    disk: Disk {
        data: [[0; BLOCK_SIZE]; NUM_BLOCKS],
    },
    next_data_block: DATA_START + 1,
});
impl FileSystem {
    pub fn new() -> Self {
        Self {
            disk: Disk::new(),
            next_data_block: DATA_START + 1,
        }
    }

    pub fn init(&mut self) {
        let root_inode = Inode {
            size: 0,
            block: DATA_START as u32,
            file_type: 2,
        };

        self.write_inode(0, root_inode);
    }

    pub fn read_inode(&self, inode_num: usize) -> Inode {
        let block = INODE_START + inode_num;

        let data = self.disk.read_block(block);

        unsafe { *(data.as_ptr() as *const Inode) }
    }

    pub fn write_inode(&mut self, inode_num: usize, inode: Inode) {
        let block = INODE_START + inode_num;

        let mut buf = [0u8; BLOCK_SIZE];

        unsafe {
            *(buf.as_mut_ptr() as *mut Inode) = inode;
        }

        self.disk.write_block(block, buf);
    }

    pub fn add_dir_entry(&mut self, name: &str, inode_num: u32) {
        let root = self.read_inode(0);

        let block = root.block as usize;

        let mut data = self.disk.read_block(block);

        let entry = DirEntry::new(name, inode_num);

        let entry_size = core::mem::size_of::<DirEntry>();

        for i in 0..(BLOCK_SIZE / entry_size) {
            let offset = i * entry_size;

            let ptr = &data[offset] as *const u8;

            let existing = unsafe {
                *(ptr as *const DirEntry)
            };

            if existing.inode == 0 {
                unsafe {
                    let dst =
                        data.as_mut_ptr().add(offset) as *mut DirEntry;

                    *dst = entry;
                }

                self.disk.write_block(block, data);

                return;
            }
        }
    }

    pub fn create(&mut self, name: &str) {
        let mut inode_num = 1;

        while inode_num < INODE_COUNT {
            let inode = self.read_inode(inode_num);

            if inode.file_type == 0 {
                break;
            }

            inode_num += 1;
        }

        if inode_num >= INODE_COUNT {
            println!("No free inodes");
            return;
        }

        let inode = Inode {
            size: 0,
            block: 0,
            file_type: 1,
        };

        self.write_inode(inode_num, inode);

        self.add_dir_entry(name, inode_num as u32);

        println!("Created file: {}", name);
    }

    pub fn find_inode(&self, name: &str) -> Option<u32> {
        let root = self.read_inode(0);

        let block = root.block as usize;

        let data = self.disk.read_block(block);

        let entry_size = core::mem::size_of::<DirEntry>();

        for i in 0..(BLOCK_SIZE / entry_size) {
            let offset = i * entry_size;

            let entry = unsafe {
                *(data.as_ptr().add(offset) as *const DirEntry)
            };

            if entry.inode != 0 {
                let name_bytes =
                    &entry.name[..entry.name_len as usize];

                if name_bytes == name.as_bytes() {
                    return Some(entry.inode);
                }
            }
        }

        None
    }

    pub fn write_file(&mut self, name: &str, content: &str) {
        let inode_num = match self.find_inode(name) {
            Some(i) => i,
            None => {
                println!("File not found");
                return;
            }
        };

        let mut inode = self.read_inode(inode_num as usize);

        let block = if inode.block == 0 {
            let b = self.next_data_block;

            self.next_data_block += 1;

            b
        } else {
            inode.block as usize
        };

        let mut buf = [0u8; BLOCK_SIZE];

        let bytes = content.as_bytes();

        let len = bytes.len().min(BLOCK_SIZE);

        for i in 0..len {
            buf[i] = bytes[i];
        }

        self.disk.write_block(block, buf);

        inode.size = len as u32;
        inode.block = block as u32;

        self.write_inode(inode_num as usize, inode);
    }

    pub fn read_file(&self, name: &str) {
        let inode_num = match self.find_inode(name) {
            Some(i) => i,
            None => {
                println!("File not found");
                return;
            }
        };

        let inode = self.read_inode(inode_num as usize);

        if inode.block == 0 {
            println!("File is empty");
            return;
        }

        let data = self.disk.read_block(inode.block as usize);

        let size = inode.size as usize;

        match core::str::from_utf8(&data[..size]) {
            Ok(content) => {
                println!("{}", content);
            }
            Err(_) => {
                println!("Invalid file data");
            }
        }
    }

    pub fn list_files(&self) {
        let root = self.read_inode(0);

        let block = root.block as usize;

        let data = self.disk.read_block(block);

        let entry_size = core::mem::size_of::<DirEntry>();

        let mut found = false;

        println!("Directory Listing");
        println!("-----------------");

        for i in 0..(BLOCK_SIZE / entry_size) {
            let offset = i * entry_size;

            let entry = unsafe {
                *(data.as_ptr().add(offset) as *const DirEntry)
            };

            if entry.inode != 0 {
                let name = core::str::from_utf8(
                    &entry.name[..entry.name_len as usize],
                )
                .unwrap();

                found = true;

                println!("{}", name);
            }
        }

        if !found {
            println!("No files found");
        }
    }

    pub fn stat(&self, name: &str) {
        if let Some(inode_num) = self.find_inode(name) {
            let inode = self.read_inode(inode_num as usize);

            println!("File: {}", name);
            println!("Inode: {}", inode_num);
            println!("Size : {} bytes", inode.size);
            println!("Block: {}", inode.block);
        } else {
            println!("File not found");
        }
    }

    pub fn delete(&mut self, name: &str) {
        if let Some(inode_num) = self.find_inode(name) {
            self.write_inode(
                inode_num as usize,
                Inode::default(),
            );

            println!("Deleted {}", name);
        } else {
            println!("File not found");
        }
    }
}