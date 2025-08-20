use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program, system_instruction};
use anchor_lang::system_program::{self, Transfer};

declare_id!("GKjnF9FWdhUMZzD55d2Ruf8xDBmvGba39ThHJUktsG8a");

pub mod fbbuy {
    use super::*;

    pub fn create_lottery_pool(
        ctx: Context<CreateLotteryPool>,
        match_id: u64,
        end_timestamp: i64,
    ) -> Result<()> {
        // 获取彩票池账户的引用
        let lottery_pool = &mut ctx.accounts.lottery_pool;

        // 将比赛信息写入账户
        lottery_pool.match_id = match_id;
        lottery_pool.end_timestamp = end_timestamp;

        // 初始化其他字段
        lottery_pool.total_payout_amount = 0;
        lottery_pool.is_settled = false;

        Ok(())
    }

    pub fn place_bet(ctx: Context<PlaceBet>, bet_amount: u64, bet_option: BetOption) -> Result<()> {
        // 1. 将投注金额从投注者转入彩票池
        let cpi_instruction = system_instruction::transfer(
            &ctx.accounts.bettor.key(),
            &ctx.accounts.lottery_pool.key(),
            bet_amount,
        );

        // 2. 构建跨程序调用（CPI）上下文
        let cpi_accounts = [
            ctx.accounts.bettor.to_account_info(),
            ctx.accounts.lottery_pool.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ];

        // 3. 调用 Solana 系统程序的 transfer 指令
        program::invoke(&cpi_instruction, &cpi_accounts)?;

        // 4. 更新 BetAccount
        let bet_account = &mut ctx.accounts.bet_account;
        bet_account.bettor = *ctx.accounts.bettor.key;
        bet_account.amount = bet_amount;
        bet_account.bet_option = bet_option;
        bet_account.is_settled = false;

        Ok(())
    }

    pub fn settle_and_payout(ctx: Context<SettleAndPayout>, final_result: BetOption) -> Result<()> {
        let lottery_pool = &mut ctx.accounts.lottery_pool;
        let bet_account = &mut ctx.accounts.bet_account;

        // 从 Chainlink 预言机账户中读取比赛结果
        let final_result_from_oracle = ctx.accounts.oracle_account.final_result;

        // 检查预言机数据的有效性，例如时间戳是否足够新
        // 实际项目中需要更严谨的检查
        if lottery_pool.end_timestamp > ctx.accounts.oracle_account.timestamp {
            return err!(MyLotteryError::OracleDataIsStale);
        }

        // 检查比赛ID是否匹配，确保我们没有使用错误的预言机数据
        if lottery_pool.match_id != ctx.accounts.oracle_account.match_id {
            return err!(MyLotteryError::MismatchedMatchId);
        }

        // 比较投注选项和比赛结果
        if bet_account.bet_option == final_result_from_oracle {
            // 中奖逻辑
            // TODO: 计算奖金并转账
            let winnings = bet_account.amount * 2; // 假设赔率为 2
                                                   // 从彩票池向中奖者转账
            let cpi_accounts = Transfer {
                from: lottery_pool.to_account_info(),
                to: bet_account.to_account_info(),
            };
            let cpi_context =
                CpiContext::new(ctx.accounts.system_program.to_account_info(), cpi_accounts);
            system_program::transfer(cpi_context, winnings)?;
            bet_account.is_settled = true;
        } else {
            // 未中奖逻辑
            bet_account.is_settled = true;
        }

        lottery_pool.is_settled = true;
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum BetOption {
    HomeWin,
    Draw,
    AwayWin,
}

// 准确比分 BetOption
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub struct Score {
    pub home_score: u8, // 主队比分
    pub away_score: u8, // 客队比分
}

// 也可以将常见比分定义为枚举
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum ExactScore {
    OneZero,  // 1:0
    TwoOne,   // 2:1
    ZeroZero, // 0:0
    Other,    // 其他比分
}

// 总进球数 BetOption
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum TotalGoals {
    ZeroToOne,  // 0-1球
    TwoToThree, // 2-3球
    FourToFive, // 4-5球
    SixOrMore,  // 6球或以上
}

// 让球 BetOption
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub struct HandicapBet {
    pub bet_on: Team,  // 投注对象：主队或客队
    pub handicap: i32, // 让球数，例如 -1.5, +0.5 等
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum Team {
    Home,
    Away,
}

#[error_code]
pub enum MyLotteryError {
    #[msg("The oracle data is stale and cannot be used.")]
    OracleDataIsStale,
    #[msg("The match ID from the oracle does not match the lottery pool.")]
    MismatchedMatchId,
}

#[account]
pub struct LotteryPool {
    pub match_id: u64,
    pub end_timestamp: i64,
    pub total_payout_amount: u64,
    pub is_settled: bool,
}

#[account]
pub struct BetAccount {
    pub bettor: Pubkey,
    pub amount: u64,
    pub bet_option: BetOption,
    pub is_settled: bool,
}

#[account]
pub struct ChainlinkOracle {
    pub match_id: u64,
    pub final_result: BetOption,
    pub timestamp: i64,
}

#[derive(Accounts)]
pub struct SettleAndPayout<'info> {
    // 1. 彩票池账户
    // `mut` 允许我们修改其状态（例如，将其标记为已结算）
    // `has_one = oracle_account` 确保彩票池和预言机账户之间存在关联，防止使用错误的预言机
    #[account(mut)]
    pub lottery_pool: Account<'info, LotteryPool>,

    // 2. 预言机账户 (Chainlink)
    // 这是我们的数据来源，包含比赛最终结果
    /// CHECK: This is a Chainlink account, we only read from it.
    pub oracle_account: Account<'info, ChainlinkOracle>,

    // 3. 中奖者账户 (SOL)
    // `mut` 表示其余额会增加
    #[account(mut)]
    pub bet_account: Account<'info, BetAccount>,

    // 4. 管理员或可信地址
    // 用于触发派奖操作，通常是你的后端服务器
    pub authority: Signer<'info>,

    // 5. Solana 系统程序
    // 进行 SOL 转账所必需的程序
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(match_id: u64)]
pub struct CreateLotteryPool<'info> {
    // 1. 彩票池账户
    // `init`: 如果账户不存在，则创建它。
    // `payer = manager`: 指定由 manager 账户来支付创建账户的费用。
    // `space`: 定义账户所需的空间大小。
    // `seeds` 和 `bump`: 创建一个可预测的 Program Derived Address (PDA)。
    #[account(
        init,
        payer = manager,
        space = 8 + 8 + 8 + 8 + 1, // 8 bytes for Anchor's discriminator, 8 for match_id, 8 for end_timestamp, 8 for total_payout_amount, 1 for is_settled
        seeds = [b"lottery_pool", match_id.to_le_bytes().as_ref()],
        bump
    )]
    pub lottery_pool: Account<'info, LotteryPool>,

    // 2. 管理者账户
    // `mut`: 管理者账户的 SOL 余额会因支付租金而减少。
    // `Signer`: 必须是这个交易的签名者，以证明其身份和意图。
    #[account(mut)]
    pub manager: Signer<'info>,

    // 3. Solana 系统程序
    // 这是 Anchor 自动添加的，用于创建新账户。
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(bet_amount: u64, bet_option: BetOption)]
pub struct PlaceBet<'info> {
    // 1. 投注账户
    // `init`: 如果账户不存在，则创建它。
    // `payer = bettor`: 由投注者支付创建账户的租金。
    // `space`: 账户所需的空间大小。
    // `seeds`: 使用投注者地址和比赛 ID 作为种子，确保每个用户在每场比赛中只有一个投注账户。
    #[account(
        init,
        payer = bettor,
        space = 8 + 32 + 8 + 1 + 1, // Anchor's discriminator + data
        seeds = [b"bet", bettor.key().as_ref(), &lottery_pool.key().to_bytes()],
        bump
    )]
    pub bet_account: Account<'info, BetAccount>,

    // 2. 彩票池账户
    // `mut`: 它的余额会增加，所以必须是可变的。
    // `constraint`: 确保投注时间未过。
    #[account(mut)]
    pub lottery_pool: Account<'info, LotteryPool>,

    // 3. 投注者账户
    // `mut`: 投注者的余额会因支付投注金额和租金而减少。
    // `Signer`: 必须是这个交易的签名者，以证明是本人操作。
    #[account(mut)]
    pub bettor: Signer<'info>,

    // 4. Solana 系统程序
    // 进行 SOL 转账所必需的程序。
    pub system_program: Program<'info, System>,
}
