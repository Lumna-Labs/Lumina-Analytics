//! Wire/row types, split by domain to match `db`'s module layout. Re-exported
//! flat here so call sites keep using `crate::models::Whatever` regardless of
//! which submodule actually defines it.

mod alert;
mod lending;
mod pool;
mod search;
mod token;
mod watchlist;
mod whale;

pub use alert::*;
pub use lending::*;
pub use pool::*;
pub use search::*;
pub use token::*;
pub use watchlist::*;
pub use whale::*;
