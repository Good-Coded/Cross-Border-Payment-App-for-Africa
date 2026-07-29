const logger = require('../utils/logger');

const AML_PROVIDER = process.env.AML_PROVIDER;
const AML_API_KEY = process.env.AML_API_KEY;

/**
 * Screen a wallet address against AML/sanctions lists.
 * Returns a structured result; placeholder logs a warning and returns not_screened.
 * Wire to a real provider (Elliptic, Chainalysis, ComplyAdvantage) by setting
 * AML_PROVIDER and AML_API_KEY environment variables.
 *
 * @param {string} walletAddress - Stellar public key to screen
 * @param {object} userDetails - { userId, email, ... }
 * @returns {{ screened: boolean, status: string, risk_level: string|null, provider: string|null, screened_at: string }}
 */
async function amlScreen(walletAddress, userDetails) {
  if (!AML_PROVIDER || !AML_API_KEY) {
    logger.warn('WARN: AML screening not configured — using passthrough', { walletAddress });
    return {
      screened: false,
      status: 'not_screened',
      risk_level: null,
      provider: null,
      screened_at: new Date().toISOString(),
    };
  }

  // Production integration point — replace with real provider SDK/HTTP call
  try {
    // Example interface (adapt per provider):
    // const result = await providerClient.screen({ address: walletAddress, ...userDetails });
    // return { screened: true, status: result.status, risk_level: result.risk_level, provider: AML_PROVIDER, screened_at: new Date().toISOString() };
    logger.warn('AML provider configured but integration not implemented — using passthrough', { provider: AML_PROVIDER });
    return {
      screened: false,
      status: 'not_screened',
      risk_level: null,
      provider: AML_PROVIDER,
      screened_at: new Date().toISOString(),
    };
  } catch (err) {
    logger.error('AML screening error', { walletAddress, provider: AML_PROVIDER, error: err.message });
    return {
      screened: false,
      status: 'not_screened',
      risk_level: null,
      provider: AML_PROVIDER,
      screened_at: new Date().toISOString(),
    };
  }
}

module.exports = { amlScreen };
