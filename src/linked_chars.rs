#[derive(Clone, Debug, PartialEq)]
pub struct CharNode {
    pub c: char,
    // Index into the arena of a LinkedChars object.
    // If None, this is the last node in the chain.
    pub next: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct LinkedChars {
    // The arena stores all nodes sequentially in memory.
    // The node at index 0 (the root) is a dummy node ('\0') and is
    // strictly NOT considered part of the actual text content.
    arena: Vec<CharNode>,
}

impl FromIterator<char> for LinkedChars {
    // Creates a new LinkedChars object from any iterator that yields characters.
    // Highly useful for parsing strings into our custom linked list structure.
    fn from_iter<I: IntoIterator<Item = char>>(iter: I) -> Self {
        let mut linked_chars = Self {
            arena: vec![CharNode {
                c: '\0',
                next: None,
            }],
        };
        let mut last_idx = 0; // Start linking from the dummy node

        for c in iter {
            let new_node = CharNode { c, next: None };
            linked_chars.arena.push(new_node);
            let newly_added_idx = linked_chars.arena.len() - 1;

            // Link the previous node to this new one
            linked_chars.get_mut(last_idx).next = Some(newly_added_idx);
            last_idx = newly_added_idx;
        }

        linked_chars
    }
}

impl LinkedChars {
    pub fn new() -> Self {
        LinkedChars {
            arena: vec![CharNode {
                c: '\0',
                next: None,
            }],
        }
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

    // Removes the sequence of nodes strictly BETWEEN start_idx and the node AFTER end_idx.
    // NOTE: start_idx MUST be the index of the node immediately PRECEDING the sequence to be removed.
    // end_idx is the last node that WILL be removed.
    pub fn remove_between(&mut self, start_idx: usize, end_idx: usize) {
        let next_after_end = self.get(end_idx).next;
        self.get_mut(start_idx).next = next_after_end;
    }

    // Replaces the sequence of nodes after start_idx up to and including end_idx
    // with the contents of another LinkedChars object.
    pub fn replace_between(&mut self, start_idx: usize, end_idx: usize, linked_chars: LinkedChars) {
        // If the new content is empty, this is equivalent to simply removing the interval
        if linked_chars.is_empty() {
            self.remove_between(start_idx, end_idx);
            return;
        }

        // Save the reference to the node AFTER the end_idx so we can link the new content to it.
        // This ensures the node at end_idx (e.g., a closing brace) is successfully dropped.
        let next_after_end = self.get(end_idx).next;

        // Pretend we just added the node at start_index to begin the linking process
        let mut last_node_added_idx = start_idx;

        // Iterate over the new text, clone it into our arena, and link it up
        for (_, node) in linked_chars.enumerate_with_start(0) {
            let mut new_node = node.clone();
            new_node.next = None;

            self.arena.push(new_node);
            let newly_added_idx = self.arena.len() - 1;

            // The node just added should be the .next of the last node added
            self.get_mut(last_node_added_idx).next = Some(newly_added_idx);
            last_node_added_idx = newly_added_idx;
        }

        // Connect the tail of the newly inserted chain to the rest of the original text
        self.arena.last_mut().unwrap().next = next_after_end;
    }

    // Returns the subinterval between start_idx and end_idx (non-inclusive of start, inclusive of end).
    pub fn interval_to_string(&self, start_idx: usize, end_idx: usize) -> String {
        let mut buffer = Vec::new();
        for (i, node) in self.enumerate_with_start(start_idx) {
            buffer.push(node.c);
            if i == end_idx {
                return buffer.into_iter().collect();
            }
        }
        panic!("end_idx was never found during interval_to_string");
    }

    pub fn make_string(&self) -> String {
        self.enumerate_with_start(0)
            .map(|(_i, node)| node.c)
            .collect::<String>()
    }

    // Returns an iterator that yields nodes starting AFTER the given start index.
    pub fn enumerate_with_start(&self, start: usize) -> LinkedCharsIter<'_> {
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
        if let Some(new_idx) = self.linked_chars.get(self.idx).next {
            self.idx = new_idx;
            Some((new_idx, self.linked_chars.get(new_idx)))
        } else {
            None
        }
    }
}

// -----------------------------------------------------------------------------
// Unit Tests for LinkedChars
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_and_is_empty() {
        // A newly created LinkedChars should be empty (only contains dummy node).
        let lc = LinkedChars::new();
        assert!(lc.is_empty(), "New LinkedChars should be empty");
        assert_eq!(
            lc.arena.len(),
            1,
            "Arena should only contain the dummy node"
        );
    }

    #[test]
    fn test_new_from_iter_and_interval_to_string() {
        // Tests if building from a string works and if we can extract it perfectly.
        // Node 0 is dummy, Node 1='a', Node 2='b', Node 3='c'.
        let lc = LinkedChars::from_iter("abc".chars());
        assert!(
            !lc.is_empty(),
            "LinkedChars should not be empty after creation from iter"
        );

        // start_idx 0 means we start reading AFTER the dummy node.
        // end_idx 3 means we stop exactly after reading 'c'.
        let result = lc.interval_to_string(0, 3);
        assert_eq!(result, "abc", "Extracted string should match input");
    }

    #[test]
    fn test_remove_between_middle() {
        // "hello" -> nodes 1(h), 2(e), 3(l), 4(l), 5(o).
        let mut lc = LinkedChars::from_iter("hello".chars());

        // Remove "ell". start_idx must be node 1 ('h'). end_idx must be node 4 (second 'l').
        // The chain should become: dummy(0) -> 'h'(1) -> 'o'(5).
        lc.remove_between(1, 4);

        // Reconstruct full string starting after dummy (0) up to the last known node (5).
        let result = lc.interval_to_string(0, 5);
        assert_eq!(result, "ho", "Expected 'ell' to be removed, leaving 'ho'");
    }

    #[test]
    fn test_replace_between_with_longer_string() {
        // "hi" -> dummy(0), 'h'(1), 'i'(2).
        let mut lc = LinkedChars::from_iter("hi".chars());
        let replacement = LinkedChars::from_iter("ello".chars());

        // Replace 'i' (node 2) with "ello".
        // start_idx is 1 ('h'), end_idx is 2 ('i').
        lc.replace_between(1, 2, replacement);

        // Since we pushed 4 new nodes to the arena, the last node index is 2 + 4 = 6.
        let result = lc.interval_to_string(0, 6);
        assert_eq!(
            result, "hello",
            "Expected 'hi' with 'i' replaced by 'ello' to yield 'hello'"
        );
    }

    #[test]
    fn test_replace_between_with_empty() {
        let mut lc = LinkedChars::from_iter("delete".chars());
        let empty_replacement = LinkedChars::new(); // Empty LinkedChars

        // Replacing "elet" (nodes 2,3,4,5) with nothing should act like remove_between.
        // start_idx: 1 ('d'), end_idx: 5 ('t'). Next node is 6 ('e').
        lc.replace_between(1, 5, empty_replacement);

        let result = lc.interval_to_string(0, 6);
        assert_eq!(result, "de", "Replacing with empty should leave 'de'");
    }

    #[test]
    #[should_panic(expected = "end_idx was never found during interval_to_string")]
    fn test_interval_to_string_panic_on_invalid_bounds() {
        let lc = LinkedChars::from_iter("abc".chars());
        // Node 99 does not exist. The function should panic to prevent silent logic errors.
        lc.interval_to_string(0, 99);
    }
}
