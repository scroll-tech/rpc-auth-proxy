use alloy::primitives::Address;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AccessLevel {
    None,
    Basic(Address),
    Full,
}

impl AccessLevel {
    pub fn is_authorized(&self, user: &Address) -> bool {
        match self {
            AccessLevel::None => false,
            AccessLevel::Basic(address) => address == user,
            AccessLevel::Full => true,
        }
    }
}
