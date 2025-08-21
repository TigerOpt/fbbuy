use anchor_lang::prelude::*;

declare_id!("GKjnF9FWdhUMZzD55d2Ruf8xDBmvGba39ThHJUktsG8a");

// 自定义错误
#[error_code]
pub enum MyLotteryError {
    #[msg("The betting period has ended.")]
    BettingPeriodEnded,
    #[msg("The lottery has already been settled.")]
    AlreadySettled,
    #[msg("You have already placed a bet for this match.")]
    AlreadyBet,
}

// 定义投注选项
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum BetOption {
    HomeWin,
    Draw,
    AwayWin,
}

// 定义彩票池账户
#[account]
pub struct LotteryPool {
    pub match_id: u64,
    pub end_timestamp: i64,
    pub total_bet_amount: u64,
    pub is_settled: bool,
    pub admin: Pubkey,
}

// 定义投注账户
#[account]
pub struct BetAccount {
    pub bettor: Pubkey,
    pub bet_amount: u64,
    pub bet_option: BetOption,
    pub lottery_pool: Pubkey,
}

#[program]
pub mod lottery {
    use super::*;

    // 指令 1: 创建彩票池
    pub fn create_lottery_pool(
        ctx: Context<CreateLotteryPool>,
        match_id: u64,
        end_timestamp: i64,
    ) -> Result<()> {
        let pool = &mut ctx.accounts.lottery_pool;
        pool.match_id = match_id;
        pool.end_timestamp = end_timestamp;
        pool.total_bet_amount = 0;
        pool.is_settled = false;
        pool.admin = *ctx.accounts.admin.key;
        Ok(())
    }

    // 指令 2: 放置投注
    pub fn place_bet(ctx: Context<PlaceBet>, bet_amount: u64, bet_option: BetOption) -> Result<()> {
        let pool = &mut ctx.accounts.lottery_pool;
        let bettor = &ctx.accounts.bettor;
        let bet_account = &mut ctx.accounts.bet_account;

        if pool.end_timestamp < Clock::get()?.unix_timestamp {
            return err!(MyLotteryError::BettingPeriodEnded);
        }

        if bet_account.bet_amount > 0 {
            return err!(MyLotteryError::AlreadyBet);
        }

        let cpi_accounts = anchor_lang::solana_program::system_instruction::transfer(
            bettor.key,
            pool.to_account_info().key,
            bet_amount,
        );
        anchor_lang::solana_program::program::invoke(
            &cpi_accounts,
            &[bettor.to_account_info(), pool.to_account_info()],
        )?;

        bet_account.bettor = *bettor.key;
        bet_account.bet_amount = bet_amount;
        bet_account.bet_option = bet_option;
        bet_account.lottery_pool = pool.key();
        pool.total_bet_amount += bet_amount;

        Ok(())
    }

    // 指令 3: 结算与派奖
    pub fn settle_and_payout(
        ctx: Context<SettleAndPayout>,
        winning_option: BetOption,
    ) -> Result<()> {
        let pool = &mut ctx.accounts.lottery_pool;

        if pool.is_settled {
            return err!(MyLotteryError::AlreadySettled);
        }

        let winner_bet_account = &mut ctx.accounts.winner_bet_account;

        // 验证投注账户与彩票池的关联
        if winner_bet_account.lottery_pool != pool.key() {
            return Err(ProgramError::InvalidAccountData.into());
        }

        // 检查投注选项是否正确
        if winner_bet_account.bet_option != winning_option {
            // 如果投注选项不正确，则不进行派奖，这通常由客户端逻辑处理
            return Ok(());
        }

        // 计算奖金
        let winning_share = winner_bet_account.bet_amount as f64 / pool.total_bet_amount as f64;
        let payout_amount = (pool.total_bet_amount as f64 * winning_share) as u64;

        // 进行派奖
        let cpi_accounts = anchor_lang::solana_program::system_instruction::transfer(
            pool.to_account_info().key,
            &winner_bet_account.bettor,
            payout_amount,
        );
        anchor_lang::solana_program::program::invoke(
            &cpi_accounts,
            &[
                pool.to_account_info(),
                ctx.accounts.winner.to_account_info(),
            ],
        )?;

        pool.is_settled = true;

        Ok(())
    }
}

// 账户上下文: 创建彩票池
#[derive(Accounts)]
#[instruction(match_id: u64)]
pub struct CreateLotteryPool<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + 8 + 8 + 8 + 1 + 32,
        seeds = [b"lottery_pool", match_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub lottery_pool: Account<'info, LotteryPool>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// 账户上下文: 投注
#[derive(Accounts)]
#[instruction(bet_amount: u64, bet_option: BetOption)]
pub struct PlaceBet<'info> {
    #[account(
        init,
        payer = bettor,
        space = 8 + 32 + 8 + 1 + 32,
        seeds = [b"bet", bettor.key().as_ref(), lottery_pool.key().as_ref()],
        bump,
    )]
    pub bet_account: Account<'info, BetAccount>,

    #[account(mut)]
    pub lottery_pool: Account<'info, LotteryPool>,

    #[account(mut)]
    pub bettor: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// 账户上下文: 结算与派奖
#[derive(Accounts)]
#[instruction(winning_option: BetOption)]
pub struct SettleAndPayout<'info> {
    #[account(mut, has_one = admin)]
    pub lottery_pool: Account<'info, LotteryPool>,

    #[account(mut)]
    pub winner_bet_account: Account<'info, BetAccount>,

    #[account(mut)]
    pub admin: Signer<'info>,

    /// CHECK: This is a winner's account.
    #[account(mut)]
    pub winner: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}
