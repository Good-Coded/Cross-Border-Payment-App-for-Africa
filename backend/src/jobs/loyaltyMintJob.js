const { v4: uuidv4 } = require('uuid');
const db = require('../db');
const logger = require('../utils/logger');
const { mintPoints } = require('../services/loyaltyToken');
const { persistAndBroadcast } = require('../services/notificationInbox');

const XLM_USD_RATE = parseFloat(process.env.XLM_USD_RATE || "0.11");

function estimateXlmEquivalent(amount, asset) {
  const num = parseFloat(amount);
  if (asset === "XLM") return num;
  if (asset === "USDC" || asset === "USD") return num / XLM_USD_RATE;
  return 0;
}

async function processLoyaltyMintQueue() {
  const CONCURRENCY = parseInt(process.env.LOYALTY_MINT_CONCURRENCY || '5', 10);

  for (let i = 0; i < CONCURRENCY; i++) {
    try {
      const { rows } = await db.query(
        `SELECT lq.id, lq.user_id, lq.sender_wallet, lq.amount, lq.asset, t.status as tx_status, t.tx_hash
         FROM loyalty_mint_queue lq
         JOIN transactions t ON t.id = lq.id
         WHERE lq.status = 'pending' AND t.status = 'completed'
         ORDER BY lq.created_at ASC
         LIMIT 1 FOR UPDATE SKIP LOCKED`,
      );

      if (!rows[0]) continue;

      const job = rows[0];
      const points = Math.max(1, Math.floor(estimateXlmEquivalent(job.amount, job.asset)));

      await db.query(
        `UPDATE loyalty_mint_queue SET status = 'processing', retry_count = retry_count + 1 WHERE id = $1`,
        [job.id],
      );

      try {
        const result = await mintPoints({ recipientWallet: job.sender_wallet, points });

        if (result) {
          await db.query(
            `UPDATE loyalty_mint_queue SET status = 'completed', tx_hash = $1, completed_at = NOW() WHERE id = $2`,
            [result.txHash, job.id],
          );

          // Record the mint in the loyalty_points off-chain ledger
          await db.query(
            `INSERT INTO loyalty_points (id, user_id, wallet_address, event_type, points, transaction_id, tx_hash)
             VALUES ($1, $2, $3, 'mint', $4, $5, $6)`,
            [uuidv4(), job.user_id, job.sender_wallet, points, job.id, result.txHash],
          );

          await persistAndBroadcast(job.user_id, 'loyalty_points_earned', 'Loyalty Points Earned',
            `You earned ${points} loyalty points for your recent payment!`,
            { points, tx_hash: result.txHash }
          ).catch(() => {});
        } else {
          await db.query(
            `UPDATE loyalty_mint_queue SET status = 'completed', completed_at = NOW() WHERE id = $1`,
            [job.id],
          );
        }
      } catch (err) {
        const retryCount = job.retry_count || 0;
        if (retryCount < 3) {
          await db.query(
            `UPDATE loyalty_mint_queue SET status = 'pending', last_error = $1, retry_count = retry_count + 1 WHERE id = $2`,
            [err.message, job.id],
          );
          logger.warn('Loyalty mint deferred, will retry', { jobId: job.id, error: err.message });
        } else {
          await db.query(
            `UPDATE loyalty_mint_queue SET status = 'failed', last_error = $1, completed_at = NOW() WHERE id = $2`,
            [err.message, job.id],
          );
          logger.error('Loyalty mint failed after max retries', { jobId: job.id, error: err.message });
        }
      }
    } catch (err) {
      logger.warn('Loyalty mint queue processing error', { error: err.message });
    }
  }
}

async function enqueueLoyaltyMint(txId, userId, senderWallet, amount, asset) {
  if (!process.env.LOYALTY_TOKEN_CONTRACT_ID) return null;

  const { rows } = await db.query(
    `INSERT INTO loyalty_mint_queue (id, user_id, sender_wallet, amount, asset)
     VALUES ($1, $2, $3, $4, $5)
     ON CONFLICT DO NOTHING
     RETURNING id`,
    [txId, userId, senderWallet, amount, asset],
  );
  return rows[0]?.id;
}

module.exports = { processLoyaltyMintQueue, enqueueLoyaltyMint };