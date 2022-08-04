use std::cmp::Ordering;
use std::iter::Peekable;

pub(crate) struct MergeAscending<L, R>
    where L: Iterator<Item=R::Item>,
          R: Iterator,
{
    left: Peekable<L>,
    right: Peekable<R>,
}

impl<L, R> MergeAscending<L, R>
    where L: Iterator<Item=R::Item>,
          R: Iterator,
{
    pub fn new(left: L, right: R) -> Self {
        MergeAscending {
            left: left.peekable(),
            right: right.peekable(),
        }
    }
}

impl<L, R> Iterator for MergeAscending<L, R>
    where L: Iterator<Item=R::Item>,
          R: Iterator,
          L::Item: Ord,
{
    type Item = L::Item;

    fn next(&mut self) -> Option<L::Item> {
        let which = match (self.left.peek(), self.right.peek()) {
            (Some(l), Some(r)) => {
                Some(l.cmp(r))
            }
            (Some(_), None) => Some(Ordering::Greater),
            (None, Some(_)) => Some(Ordering::Less),
            (None, None) => None,
        };

        match which {
            Some(Ordering::Greater) => self.left.next(),
            Some(Ordering::Equal) => self.left.next(),
            Some(Ordering::Less) => self.right.next(),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::search_thread::merge::MergeAscending;

    #[test]
    fn find_value_in_text() {
        let x1 = &vec![3, 2, 1];
        let x2 = &vec![6, 5, 4];


        let ascending = MergeAscending::new(x1.iter(), x2.iter());
        let x5: Vec<_> = ascending.collect();
        println!("{:?}", x5);
    }
}