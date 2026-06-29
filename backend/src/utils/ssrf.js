/**
 * SSRF protection utility.
 * Validates outbound URLs to prevent Server-Side Request Forgery attacks.
 * Used by webhook registration, delivery, and SEP-31 callback endpoints.
 */

const dns = require('dns').promises;
const { isPrivateIp } = require('./ssrfValidator');

// Parse optional trusted CIDR allowlist from environment
// Format: comma-separated CIDR strings, e.g. "10.0.0.0/8,192.168.1.0/24"
function parseAllowedCidrs() {
  const raw = process.env.WEBHOOK_ALLOWED_CIDRS || '';
  return raw.split(',').filter(Boolean).map(cidr => {
    const [ip, bits] = cidr.trim().split('/');
    const prefix = parseInt(bits || '32', 10);
    const mask = prefix === 0 ? 0 : (~0 << (32 - prefix)) >>> 0;
    const net = ip.split('.').reduce((a, o) => (a << 8) + parseInt(o, 10), 0) >>> 0;
    return { net: net & mask, mask };
  });
}

function ipToInt(ip) {
  return ip.split('.').reduce((acc, o) => (acc << 8) + parseInt(o, 10), 0) >>> 0;
}

function isAllowlisted(ip, allowedCidrs) {
  if (!allowedCidrs.length) return false;
  if (!/^\d{1,3}(\.\d{1,3}){3}$/.test(ip)) return false;
  const n = ipToInt(ip);
  return allowedCidrs.some(({ net, mask }) => (n & mask) === net);
}

const ALLOWED_SCHEMES = new Set(['https:', 'http:']);

/**
 * Validate that an outbound URL is safe to request.
 *
 * Rejects:
 * - Non-HTTP(S) schemes (ftp, file, gopher, etc.)
 * - URLs resolving to private/loopback/link-local IPs
 * - Unresolvable hostnames
 *
 * Allows:
 * - Public HTTPS/HTTP URLs
 * - IPs in WEBHOOK_ALLOWED_CIDRS (for self-hosted/test environments)
 *
 * @param {string} url
 * @returns {Promise<{ valid: boolean, error?: string }>}
 */
async function validateOutboundUrl(url) {
  let parsed;
  try {
    parsed = new URL(url);
  } catch {
    return { valid: false, error: 'Invalid URL format' };
  }

  if (!ALLOWED_SCHEMES.has(parsed.protocol)) {
    return { valid: false, error: `Scheme "${parsed.protocol}" is not allowed. Only http and https are permitted.` };
  }

  const hostname = parsed.hostname;
  const allowedCidrs = parseAllowedCidrs();

  // Reject bare private IPs unless allowlisted
  if (/^\d{1,3}(\.\d{1,3}){3}$/.test(hostname)) {
    if (isPrivateIp(hostname) && !isAllowlisted(hostname, allowedCidrs)) {
      return { valid: false, error: 'SSRF_BLOCKED' };
    }
    return { valid: true };
  }

  // Resolve hostname and check all returned IPs
  try {
    const { address } = await dns.lookup(hostname);
    if (isPrivateIp(address) && !isAllowlisted(address, allowedCidrs)) {
      return { valid: false, error: 'SSRF_BLOCKED' };
    }
  } catch {
    return { valid: false, error: 'Hostname could not be resolved' };
  }

  return { valid: true };
}

module.exports = { validateOutboundUrl };
