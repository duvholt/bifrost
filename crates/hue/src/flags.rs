pub trait TakeFlag {
    fn take(&mut self, flag: Self) -> bool;
}

impl<T: bitflags::Flags + Copy> TakeFlag for T {
    fn take(&mut self, flag: Self) -> bool {
        let found = self.contains(flag);
        if found {
            self.remove(flag);
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use bitflags::bitflags;

    use crate::flags::TakeFlag;

    bitflags! {
        #[derive(Debug, Clone, Copy)]
        pub struct Flags: u16 {
            const BIT  = 1;
        }
    }

    #[test]
    fn take_none() {
        let mut fl = Flags::from_bits(0).unwrap();
        assert!(!fl.take(Flags::BIT));
    }

    #[test]
    fn take_one() {
        let mut fl = Flags::from_bits(1).unwrap();
        assert!(fl.take(Flags::BIT));
    }

    #[test]
    fn take_twice() {
        let mut fl = Flags::from_bits(1).unwrap();
        assert!(fl.take(Flags::BIT));
        assert!(!fl.take(Flags::BIT));
    }
}
