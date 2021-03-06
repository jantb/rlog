use std::iter::Peekable;
use std::cmp::Ordering;

pub(crate) struct MergeAscending<L, R>
    where L: Iterator<Item = R::Item>,
          R: Iterator,
{
    left: Peekable<L>,
    right: Peekable<R>,
}

impl<L, R> MergeAscending<L, R>
    where L: Iterator<Item = R::Item>,
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
    where L: Iterator<Item = R::Item>,
          R: Iterator,
          L::Item: Ord,
{
    type Item = L::Item;

    fn next(&mut self) -> Option<L::Item> {
        let which = match (self.left.peek(), self.right.peek()) {
            (Some(l), Some(r)) => Some(l.cmp(r)),
            (Some(_), None)    => Some(Ordering::Less),
            (None, Some(_))    => Some(Ordering::Greater),
            (None, None)       => None,
        };

        match which {
            Some(Ordering::Greater)    => self.left.next(),
            Some(Ordering::Equal)   => self.left.next(),
            Some(Ordering::Less) => self.right.next(),
            None                    => None,
        }
    }
}
macro_rules! merging_iterator_from {  
    ($vv: ident) => {{    
        let mut ma: Box<dyn Iterator<Item=_>> = Box::new($vv[0].iter().map(|&i|i));
        for v in $vv.iter().skip(1) {
            ma = Box::new(merge::MergeAscending::new(ma, v.iter().map(|&i|i)));
        };
        ma
    }}
}
pub(crate) use merging_iterator_from; 