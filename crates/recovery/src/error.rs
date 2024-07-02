#[derive(Debug, PartialEq, Eq)]
pub enum RecoveryErr {
    ExtaustiveK,
    InvalidLength,
    RowIdOverflow,
    TooFewRowIds,
}
