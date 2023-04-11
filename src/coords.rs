//! Contains coordinate structs, QuadVec for quadtrees, and OctVec for octrees, as well as their LodVec implementation

use std::cmp::Ordering;

pub trait LodVec<const N: usize>:
    std::hash::Hash + Eq + Sized + Copy + Clone + Send + Sync + std::fmt::Debug + PartialOrd
{
    const MAX_CHILDREN: usize = 1 << N;
    //const GGG:usize = N.l
    /// gets one of the child node position of this node, defined by it's index.
    fn get_child(self, index: usize) -> Self;

    /// returns index of child for a given child position (reciprocal of get_child)
    fn get_child_index(self, child: Self) -> usize;

    /// the number of child nodes a node can have in the tree.
    fn contains_child_node(self, child: Self) -> bool;

    /// returns the lod vector as if it's at the root of the tree.
    fn root() -> Self;

    /// wether a target with this position can subdivide a given node, allowing for required "detail" region.
    ///
    /// Assumes self is the target position for a lod.
    ///
    /// self.depth determines the max lod level allowed, detail determines the amount of chunks around the target.
    ///
    /// if the detail is 0, this may only return true if self is inside the node.
    ///
    fn can_subdivide(self, node: Self, detail: u32) -> bool;

    /// check if this chunk is inside of a bounding box
    /// where min is the lowest corner of the box, and max is the highest corner
    /// max_depth controls the depth at which the BB checking is done.
    /// ```
    fn is_inside_bounds(self, min: Self, max: Self, max_depth: u8) -> bool;

    /// Retrieve current depth
    fn depth(self) -> u8;
}

pub trait ReasonableIntegerLike:
    Default
    + num::Integer
    + std::marker::Send
    + std::marker::Sync
    + std::fmt::Debug
    + Copy
    + std::hash::Hash
    + std::ops::Shl<isize, Output = Self>
    + std::ops::Shr<isize, Output = Self>
    + std::ops::Shl<usize, Output = Self>
    + std::ops::Shr<usize, Output = Self>
    + std::ops::BitAnd<Self, Output = Self>
{
    fn fromusize(value: usize) -> Self;
    fn tousize(self) -> usize;
}

#[macro_export]
macro_rules! reasonable_int_impl {
    (  $x:ty  ) => {
        impl ReasonableIntegerLike for $x {
            fn fromusize(value: usize) -> Self {
                value as $x
            }
            fn tousize(self) -> usize {
                self as usize
            }
        }
    };
}

reasonable_int_impl!(u8);
reasonable_int_impl!(u16);
reasonable_int_impl!(u32);
reasonable_int_impl!(u64);

#[cfg(test)]
mod tests {
    use crate::coords::*;
    use std::mem::size_of;

    #[test]
    fn sizes() {
        assert_eq!(3, size_of::<QuadVec>());
        assert_eq!(4, size_of::<OctVec>());
    }
    #[test]
    fn find_child_idx() {
        // create root
        let z = OctVec::<u8>::root();
        // loop over possible children
        for i in 0..OctVec::<u8>::MAX_CHILDREN {
            // get child of z with index i
            let c = z.get_child(i);
            // recover its index based on coords
            let ci = z.get_child_index(c);
            // make sure they are identical
            assert_eq!(ci, i);

            for j in 0..OctVec::<u8>::MAX_CHILDREN {
                // get child of c
                let cc = c.get_child(j);
                // and its index
                let cci = c.get_child_index(cc);
                println!("{}->{} ({}->{}): {:?}->{:?} ", i, j, ci, cci, c, cc);
                assert_eq!(cci, j);
                // we can also get index w.r.t. previous levels
                let czi = z.get_child_index(cc);
                assert_eq!(czi, i);
                // and we can go deeper too...
                for k in 0..OctVec::<u8>::MAX_CHILDREN {
                    let ccc = cc.get_child(k);
                    assert_eq!(z.get_child_index(ccc), i);
                    assert_eq!(c.get_child_index(ccc), j);
                    assert_eq!(cc.get_child_index(ccc), k);
                }
            }
        }
    }

    #[test]
    fn can_subdivide() {
        let z: QuadVec = QuadVec::root();
        let c1 = z.get_child(0);
        let c12 = c1.get_child(0);
        let tgt = QuadVec::build(0, 0, 2);

        println!("{tgt:?}, {c12:?}, {}", tgt.can_subdivide(c12, 3));
        println!("{tgt:?}, {c1:?}, {}", tgt.can_subdivide(c1, 3));
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, std::hash::Hash)]
pub struct CoordVec<const N: usize, DT = u8>
where
    DT: ReasonableIntegerLike,
{
    pub pos: [DT; N],
    pub depth: u8,
}

impl<const N: usize, DT> CoordVec<N, DT>
where
    DT: ReasonableIntegerLike,
{
    /// creates a new vector from the raw x and y coords.
    /// # Args
    /// * `coord` The position in the tree. Allowed range scales with the depth (doubles as the depth increases by one)
    /// * `depth` the lod depth the coord is at. This is soft limited at roughly 60, and the tree might behave weird if it gets higher
    #[inline]
    pub fn new(pos: [DT; N], depth: u8) -> Self {
        debug_assert!(depth <= 60);
        debug_assert!(
            pos.iter().all(|e| { e.tousize() < (1 << depth) }),
            "All components of position should be < 2^depth"
        );

        Self { pos, depth }
    }

    /// creates a new vector from floating point coords.
    /// mapped so that e.g. (0, 0, 0) is the front bottom left corner and (1, 1, 1) is the back top right.
    /// # Args
    /// * `pos` coordinates of the float vector, from 0 to 1
    /// * `depth` The lod depth of the coord
    #[inline]
    pub fn from_float_coords(pos: [f32; N], depth: u8) -> Self {
        // scaling factor due to the lod depth
        let scale_factor = (1 << depth) as f32;

        // and get the actual coord
        Self {
            pos: pos.map(|e| DT::fromusize((e * scale_factor) as usize)),
            depth,
        }
    }

    /// converts the coord into float coords.
    /// Returns a slice of f64 to represent the coordinates, at the front bottom left corner.
    #[inline]
    pub fn float_coords(self) -> [f32; N] {
        // scaling factor to scale the coords down with
        let scale_factor = 1.0 / (1 << self.depth) as f32;
        self.pos.map(|e| e.tousize() as f32 * scale_factor)
    }

    /// gets the size the chunk of this lod vector takes up, with the root taking up the entire area.
    #[inline]
    pub fn float_size(self) -> f32 {
        1.0 / (1 << self.depth) as f32
    }
}

impl<const N: usize, DT> LodVec<N> for CoordVec<N, DT>
where
    DT: ReasonableIntegerLike,
{
    fn root() -> Self {
        Self {
            pos: [DT::default(); N],
            depth: 0,
        }
    }

    fn depth(self) -> u8 {
        return self.depth;
    }

    #[inline]
    fn get_child(self, index: usize) -> Self {
        debug_assert!(index < <CoordVec<N> as LodVec<N>>::MAX_CHILDREN);
        let mut new = Self::root();
        //println!("GetChild for {:?} idx {}", self,index);
        for i in 0..N {
            let p_doubled = self.pos[i].tousize() << 1;

            let p = p_doubled + ((index & (1 << i)) >> i);
            //dbg!(i, p_doubled, p);
            new.pos[i] = DT::fromusize(p);
        }
        new.depth = self.depth + 1;
        new
    }

    fn get_child_index(self, child: Self) -> usize {
        debug_assert!(self.depth < child.depth);
        let level_difference = child.depth - self.depth;
        //let one = DT::fromusize(1 as usize);
        let mut idx: usize = 0;
        for i in 0..N {
            //scale up own base pos
            let sp = self.pos[i].tousize() << level_difference;
            let pi = (child.pos[i].tousize() - sp) >> (level_difference - 1);
            //dbg!(i, sp, pi);
            idx |= pi << i;
        }
        idx
    }

    fn contains_child_node(self, child: Self) -> bool {
        // basically, move the child node up to this level and check if they're equal
        let level_difference = child.depth as isize - self.depth as isize;
        self.pos
            .iter()
            .zip(child.pos)
            .all(|(s, c)| *s == (c >> level_difference))
    }

    fn is_inside_bounds(self, min: Self, max: Self, max_depth: u8) -> bool {
        // get the lowest lod level
        let level = *[self.depth, min.depth, max.depth].iter().min().unwrap() as isize;
        //dbg!(level);
        // bring all coords to the lowest level
        let self_difference: isize = self.depth as isize - level;
        let min_difference: isize = min.depth as isize - level;
        let max_difference: isize = max.depth as isize - level;
        //println!("diff {:?},  {:?}, {:?}", self_difference, min_difference, max_difference);
        // get the coords to that level

        let self_lowered = self.pos.map(|e| e >> self_difference);
        let min_lowered = min.pos.map(|e| e >> min_difference);
        let max_lowered = max.pos.map(|e| e >> max_difference);
        //println!("lowered {self_lowered:?},  {min_lowered:?}, {max_lowered:?}");
        // then check if we are inside the AABB
        self.depth <= max_depth
            && itertools::izip!(self_lowered, min_lowered, max_lowered)
                .all(|(slf, min, max)| slf >= min && slf <= max)
    }

    #[inline]
    fn can_subdivide(self, node: Self, detail: u32) -> bool {
        let detail = detail as usize;
        // return early if the level of this chunk is too high
        if node.depth >= self.depth {
            return false;
        }

        // difference in lod level between the target and the node
        let level_difference = self.depth - node.depth;

        // minimum corner of the bounding box
        let min = node.pos.iter().map(|e| {
            let x = e.tousize();
            let x = (x << (level_difference + 1))
                .saturating_sub(((detail + 1) << level_difference) - (1 << level_difference));
            DT::fromusize(x)
        });

        // maximum corner of the bounding box
        let max = node.pos.iter().map(|e| {
            let x = e.tousize();
            let x = (x << (level_difference + 1))
                .saturating_add(((detail + 1) << level_difference) + (1 << level_difference));
            DT::fromusize(x)
        });

        // iterator over bounding boxes
        let minmax = min.zip(max);

        // local position of the target, moved one lod level higher to allow more detail
        let local = self.pos.iter().map(|e| *e << 1usize);
        //println!("Check tgt {self:?} wrt {node:?}");
        // check if the target is inside of the bounding box
        local.zip(minmax).all(|(c, (min, max))| {
          //  println!("{min:?} <= {c:?} < {max:?}");
            min <= c && c < max}
        )
    }
}

pub type OctVec<DT = u8> = CoordVec<3, DT>;
pub type QuadVec<DT = u8> = CoordVec<2, DT>;

impl<DT> OctVec<DT>
where
    DT: ReasonableIntegerLike,
{
    pub fn build(x: DT, y: DT, z: DT, depth: u8) -> Self {
        Self::new([x, y, z],depth)
    }
}

impl<DT> QuadVec<DT>
where
    DT: ReasonableIntegerLike,
{
    pub fn build(x: DT, y: DT, depth: u8) -> Self {
        Self::new([x, y],depth)
    }
}

impl<const N: usize, DT> PartialOrd for CoordVec<N, DT>
where
    DT: ReasonableIntegerLike,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.depth != other.depth {
            return None;
        }

        if self.pos == other.pos {
            return Some(Ordering::Equal);
        }

        if self.pos.iter().zip(other.pos).all(|(s, o)| *s < o) {
            return Some(Ordering::Less);
        } else if self.pos.iter().zip(other.pos).all(|(s, o)| *s > o) {
            return Some(Ordering::Greater);
        }
        None
    }
}

impl<const N: usize, DT> std::ops::Add for CoordVec<N, DT>
where
    DT: ReasonableIntegerLike,
{
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        debug_assert_eq!(self.depth, rhs.depth);
        let mut res = self.clone();
        for (e1, e2) in res.pos.iter_mut().zip(rhs.pos) {
            *e1 = *e1 + e2;
        }
        res
    }
}

impl<const N: usize, DT> Default for CoordVec<N, DT>
where
    DT: ReasonableIntegerLike,
{
    fn default() -> Self {
        Self::root()
    }
}

pub fn get_chunk_count_at_max_depth<const N: usize>(a: CoordVec<N>, b: CoordVec<N>) -> usize {
    assert_eq!(a.depth, b.depth);
    b.pos
        .iter()
        .zip(a.pos)
        .fold(1, |acc, (e1, e2)| acc * (e1 - e2 + 1) as usize)
}

//#[cfg(feature = "rand")]
pub fn rand_cv<const N: usize, R: rand::Rng, T>(
    rng: &mut R,
    min: CoordVec<N, T>,
    max: CoordVec<N, T>,
) -> CoordVec<N, T>
where
    T: ReasonableIntegerLike + rand::distributions::uniform::SampleUniform,
{
    debug_assert_eq!(min.depth, max.depth);
    let mut zz = [T::fromusize(0); N];
    for i in 0..N {
        zz[i] = rng.gen_range(min.pos[i]..max.pos[i]);
    }
    CoordVec::new(zz, min.depth)
}
