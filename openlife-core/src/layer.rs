/// Runtime contract layer label.
///
/// This is a pure metadata type used by runtime contracts and evidence. It is
/// not a router and must not decide Main Chat product routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Reflex layer metadata.
    L1,
    /// Tactical layer metadata.
    L2,
    /// Strategic layer metadata.
    L3,
}
