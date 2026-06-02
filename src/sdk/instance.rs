#![allow(dead_code)]

use std::sync::Arc;

use crate::core::memory::{is_valid_address, Memory};
use crate::core::offsets::instance;

const MAX_CHILDREN: usize = 1000;

pub struct Instance {
    pub address: u64,
    memory: Arc<Memory>,
}

impl Instance {
    pub fn new(address: u64, memory: Arc<Memory>) -> Self {
        Self { address, memory }
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        is_valid_address(self.address)
    }

    pub fn get_name(&self) -> String {
        let name_ptr = self.memory.read::<u64>(self.address + instance::name());
        if is_valid_address(name_ptr) {
            self.memory.read_string(name_ptr)
        } else {
            String::new()
        }
    }

    pub fn get_class_name(&self) -> String {
        let descriptor = self.memory.read::<u64>(self.address + instance::class_descriptor());
        let class_name = self.memory.read::<u64>(descriptor + instance::class_name());

        if class_name != 0 {
            self.memory.read_string(class_name)
        } else {
            String::new()
        }
    }

    pub fn get_parent(&self) -> Option<Instance> {
        let parent_addr = self.memory.read::<u64>(self.address + instance::parent());
        if is_valid_address(parent_addr) {
            Some(Instance::new(parent_addr, Arc::clone(&self.memory)))
        } else {
            None
        }
    }

    pub fn get_children(&self) -> Vec<Instance> {
        let start = self.memory.read::<u64>(self.address + instance::children_start());

        if !is_valid_address(start) {
            return Vec::new();
        }

        let end = self.memory.read::<u64>(start + instance::children_end());
        let mut children = Vec::with_capacity(32);
        let mut current = self.memory.read::<u64>(start);
        let mut iterations = 0;

        while current != end && iterations < MAX_CHILDREN {
            let instance_addr = self.memory.read::<u64>(current);
            if is_valid_address(instance_addr) {
                children.push(Instance::new(instance_addr, Arc::clone(&self.memory)));
            }
            current += std::mem::size_of::<usize>() as u64;
            iterations += 1;
        }

        children
    }

    pub fn find_first_child(&self, name: &str) -> Option<Instance> {
        self.get_children()
            .into_iter()
            .find(|child| child.get_name().eq_ignore_ascii_case(name))
    }

    pub fn find_first_child_by_class(&self, class_name: &str) -> Option<Instance> {
        self.get_children()
            .into_iter()
            .find(|child| child.get_class_name().eq_ignore_ascii_case(class_name))
    }

    pub fn find_children<F>(&self, predicate: F) -> Vec<Instance>
    where
        F: Fn(&Instance) -> bool,
    {
        self.get_children()
            .into_iter()
            .filter(|child| predicate(child))
            .collect()
    }

    pub fn memory(&self) -> &Arc<Memory> {
        &self.memory
    }
}

impl Clone for Instance {
    fn clone(&self) -> Self {
        Self {
            address: self.address,
            memory: Arc::clone(&self.memory),
        }
    }
}
