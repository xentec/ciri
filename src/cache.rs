#![allow(dead_code)]

use std::collections::{VecDeque,HashSet};
use std::hash::Hash;

#[derive(Debug, Serialize, Deserialize)]
pub struct Queue<Item: Hash + Eq + Copy>
{
	queue: VecDeque<Item>,
	#[serde(skip)]
	lookup: HashSet<Item>,
}

impl<Item> Queue<Item>
	where Item: Hash + Eq + Copy
{
	pub fn new() -> Queue<Item> 
	{
		Queue {
			queue: VecDeque::new(),
			lookup: HashSet::new(),
		}
	}

	pub fn with_capacity(capacity: usize) -> Queue<Item> 
	{
		let mut q = Self::new();
		q.reserve(capacity);

		q
		}

	pub fn reserve(&mut self, capacity: usize)
	{
		self.queue.reserve(capacity);
		self.lookup.reserve(capacity);
	}

	pub fn len(&self) -> usize
	{
		self.queue.len()
	}

	pub fn push(&mut self, item: Item) -> Option<Item>
	{
		let mut dropped = None;
		if self.queue.len() == self.queue.capacity()
		{
			let last = self.queue.pop_front().unwrap();
			self.lookup.remove(&last);
			dropped = Some(last);
		}
		
		self.queue.push_back(item);
		self.lookup.insert(item);

		dropped
	}

	pub fn contains(&self, item: &Item) -> bool 
	{
		self.lookup.contains(item)
	}

	pub fn clear(&mut self) 
	{
		self.queue.clear();
		self.lookup.clear();
	}

	pub fn optimize(&mut self)
	{
		self.lookup.clear();
		for e in &self.queue
		{
			self.lookup.insert(*e);
		}
	}
}
/*
use std::fmt;

impl<Item> fmt::Display for Queue<Item> 
	where Item: fmt::Display + Hash + Eq + Copy
{
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result 
	{
		write!(f, "[");
		write!(f, "{}", self.queue)

	} 	
}
*/