use anchor_lang::{prelude::*, solana_program::clock::SECONDS_PER_DAY};

use crate::error::ErrorCode;

#[account]
#[derive(InitSpace)]
pub struct Oracle {
    pub incentives: u64, // incentives per update the oracle
    pub last_update: i64,
    pub approve: bool, // approve to transfer = true, if not = false
    pub oracle_vault_bump: u8,
    pub bump: u8,
}

impl Oracle {
    pub const LEN: usize = 8 + Oracle::INIT_SPACE;

    pub fn update(&mut self) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;

        let second_in_day = now.checked_rem(SECONDS_PER_DAY as i64).unwrap();
        let utc_hour = second_in_day.checked_div(3600).unwrap(); // 3600 = secs per hour

        // if the user want to set the same state, it'll throw err
        // self.last_update == 0 for the first udpate
        if utc_hour >= 9 && utc_hour < 17 && (!self.approve || self.last_update == 0) {
            self.approve = true;
        } else if self.approve || self.last_update == 0 {
            self.approve = false;
        } else {
            return Err(ErrorCode::InvalidTimestamp.into());
        }

        self.last_update = now;

        Ok(())
    }
}
