#[derive(Clone, Debug)]
pub struct CharNode {
    pub c: char,
    pub next: Option<usize>, // Index into the arena of a LinkedChars object
}

#[derive(Clone, Debug)]
pub struct LinkedChars {
    // The node at index 0 (the root) is a dummy node and is not considered part of the content.
    arena: Vec<CharNode>,
}

impl LinkedChars {
    // Creates a new, empty LinkedChars instance with a dummy root node.
    pub fn new() -> Self {
        Self {
            arena: vec![CharNode {
                c: '\0',
                next: None,
            }],
        }
    }

    // Creates a new LinkedChars object from any iterator that yields characters.
    // This is highly useful for initializing the interpreter with a string of code.
    pub fn new_from_iter<I: IntoIterator<Item = char>>(iter: I) -> Self {
        // Start with a fresh, empty LinkedChars instance (which contains the dummy root node)
        let mut linked_chars = Self::new();

        // Keep track of the last node we added, starting with the dummy node at index 0
        let mut last_idx = 0;

        for c in iter {
            let new_node = CharNode {
                c,
                next: None, // Will be updated when the next char is added
            };

            // Push the new character into the arena
            linked_chars.arena.push(new_node);
            let newly_added_idx = linked_chars.arena.len() - 1;

            // Link the previously added node to this new node
            linked_chars.get_mut(last_idx).next = Some(newly_added_idx);

            // Update our tracker to the node we just added
            last_idx = newly_added_idx;
        }

        linked_chars
    }

    // Checks if the linked list has no content (only the dummy node exists).
    pub fn is_empty(&self) -> bool {
        self.arena[0].next.is_none()
    }

    pub fn get(&self, idx: usize) -> &CharNode {
        &self.arena[idx]
    }

    pub fn get_mut(&mut self, idx: usize) -> &mut CharNode {
        &mut self.arena[idx]
    }

    // Removes the sequence of nodes after start_idx up to and including end_idx.
    // start_idx MUST be the index of the node immediately PRECEDING the sequence to be removed.
    pub fn remove_between(&mut self, start_idx: usize, end_idx: usize) {
        let next_after_end = self.get(end_idx).next;
        self.get_mut(start_idx).next = next_after_end;
    }

    // Replaces the sequence of nodes after start_idx up to and including end_idx
    // with the contents of another LinkedChars object.
    pub fn replace_between(&mut self, start_idx: usize, end_idx: usize, linked_chars: LinkedChars) {
        if linked_chars.is_empty() {
            // If the new content is empty, just remove the specified interval
            self.remove_between(start_idx, end_idx);
            return;
        }

        // Save the reference to the node AFTER the end_idx so we can link the new content to it.
        // This ensures the node at end_idx (e.g., a closing brace) is successfully dropped.
        let next_after_end = self.get(end_idx).next;

        let mut last_node_added_idx = start_idx;

        // We use the normal iterator and clone the nodes instead of a custom owned iterator.
        for (_, node) in linked_chars.enumerate_with_start(0) {
            let mut new_node = node.clone();
            new_node.next = None; // Reset the next pointer, as we manage it manually below

            self.arena.push(new_node);
            let newly_added_idx = self.arena.len() - 1;

            // Link the previously added node to the new node
            self.get_mut(last_node_added_idx).next = Some(newly_added_idx);

            // Update the tracker for the next iteration
            last_node_added_idx = newly_added_idx;
        }

        // Link the very last node of the new text to the node AFTER the replaced segment
        self.arena.last_mut().unwrap().next = next_after_end;
    }

    // Returns the substring between start_idx and end_idx (inclusive of end_idx).
    pub fn interval_to_string(&self, start_idx: usize, end_idx: usize) -> String {
        let mut buffer = Vec::new();
        // The iterator yields nodes starting AFTER start_idx.
        for (i, node) in self.enumerate_with_start(start_idx) {
            buffer.push(node.c);
            if i == end_idx {
                return buffer.into_iter().collect();
            }
        }
        panic!("end_idx was never found during interval_to_string");
    }

    // Returns an iterator that yields nodes starting AFTER the given start index.
    pub fn enumerate_with_start(&self, start: usize) -> LinkedCharsIter {
        LinkedCharsIter {
            linked_chars: self,
            idx: start,
        }
    }
}

pub struct LinkedCharsIter<'a> {
    linked_chars: &'a LinkedChars,
    idx: usize,
}

impl<'a> Iterator for LinkedCharsIter<'a> {
    type Item = (usize, &'a CharNode);

    fn next(&mut self) -> Option<Self::Item> {
        // We look at the 'next' pointer of the current node to find the new index.
        if let Some(new_idx) = self.linked_chars.get(self.idx).next {
            self.idx = new_idx;
            // Return the newly found index and its corresponding node
            Some((new_idx, self.linked_chars.get(new_idx)))
        } else {
            None
        }
    }
}
