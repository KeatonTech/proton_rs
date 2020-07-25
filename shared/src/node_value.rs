/// An RGB color with an alpha channel. Supports 16-bits per channel to allow for HDR
/// content or colors on devices like RGB LEDs that may have color accuracy beyond that
/// of most monitors.
pub type NodeColor = (u16, u16, u16, u16);

/// Proton-specific data type representation.
#[derive(Debug, EnumDiscriminants, PartialEq, Clone)]
#[strum_discriminants(name(NodeValueType))]
pub enum NodeValue {
    /// Stateless value, acts as a way of kicking off an action.
    Trigger(),

    /// Boolean value, used to switch something on or off
    Toggle(bool),

    /// Signed integer value. Acts just like i64 in Rust.
    Count(i64),

    /// Represents a value from 0 to 1 with a precision of 1/(2^32).
    ConstrainedMagnitude(u32),

    /// Like ConstrainedMagnitude, meant to represent a value of 0 to 1. Unlike
    /// ConstrainedMagnitude it is actually able to go outside of those bounds, allowing
    /// the value to be inverted (passed a value < 0) or 'over-driven' (passed a value > 1).
    UnconstrainedMagnitude(f64),

    /// An RGB color with an alpha channel. Supports 16-bits per channel to allow for HDR
    /// content or colors on devices like RGB LEDs that may have color accuracy beyond that
    /// of most monitors.
    Color(NodeColor),

    /// UTF-8 string data.
    Text(Box<String>),

    /// 1-dimensional bitmap image. Stored uncompressed.
    Bitmap1D(Box<Vec<NodeColor>>),

    /// 2-dimensional bitmap image. Stored uncompressed.
    Bitmap2D(Box<Vec<Vec<NodeColor>>>),

    /// Shader program with a 1-dimensional positional input. Stores the index of the program,
    /// not the program itself, so that this value can be comparable and clonable.
    Shader1D(u16),

    /// Shader program with a 2-dimensional positional input. Stores the index of the program,
    /// not the program itself, so that this value can be comparable and clonable.
    Shader2D(u16),

    /// Shader program with a 3-dimensional positional input. Stores the index of the program,
    /// not the program itself, so that this value can be comparable and clonable.
    Shader3D(u16),
}