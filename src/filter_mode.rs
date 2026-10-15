#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FilterMode {
    #[default]
    New,
    Edit {
        ids: Vec<u64>,
        selected: u64,
    },
}
impl FilterMode {
    pub fn ids(&self) -> &[u64] {
        match self {
            Self::New => &[],
            Self::Edit { ids, .. } => ids,
        }
    }
    pub fn selected(&self) -> Option<u64> {
        match self {
            Self::New => None,
            Self::Edit { selected, .. } => Some(*selected),
        }
    }
    pub fn is_new(&self) -> bool {
        matches!(self, Self::New)
    }
}
