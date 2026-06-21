/// The type of a directed edge in the social graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationKind {
    Follow,
    Block,
}
