const https = require('https');
const db = require('../db');
const { sign } = require('../utils/webhookSignature');
const { validateOutboundUrl } = require('../utils/ssrf');
const { decryptSecret } = require('../utils/symmetricEncryption');

const MAX_ATTEMPTS = 3;

/**
 * Check if a URL is a valid public HTTPS endpoint
 * Re-validates before each delivery to catch DNS rebinding attacks
 */
async function isPublicHttpsUrl(url) {
  let parsed;
  try {
    parsed = new URL(url);
  } catch {
    return false;
  }

  if (parsed.protocol !== 'https:') {
    return false;
  }

  const ssrfCheck = await validateOutboundUrl(url);
  if (!ssrfCheck.valid) {
    return false;
  }

  return true;
}

function httpsPost(url, body, signature, agent) {
  return new Promise((resolve, reject) => {
    const parsed = new URL(url);
    const options = {
      hostname: parsed.hostname,
      port: parsed.port || 443,
      path: parsed.pathname + parsed.search,
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Content-Length': Buffer.byteLength(body),
        'X-AfriPay-Signature-256': `sha256=${signature}`,
      },
      // Use the DNS-pinned agent from SSRF validation to prevent rebinding
      ...(agent && { agent }),
    };
    const req = https.request(options, (res) => {
      res.resume();
      // Block redirects to prevent DNS rebinding via 3xx responses
      if (res.statusCode >= 300 && res.statusCode < 400) {
        return reject(new Error(`Redirect blocked (HTTP ${res.statusCode}) — follow redirects is disabled for security`));
      }
      res.statusCode >= 200 && res.statusCode < 300 ? resolve(res.statusCode) : reject(new Error(`HTTP ${res.statusCode}`));
    });
    req.on('error', reject);
    req.write(body);
    req.end();
  });
}

async function createDeliveryLog(webhookId, eventType, targetUrl, attempt, maxAttempts, payload) {
  const { rows } = await db.query(
    `INSERT INTO webhook_deliveries (webhook_id, event_type, target_url, status, attempt, max_attempts, payload)
     VALUES ($1, $2, $3, $4, $5, $6, $7)
     RETURNING id`,
    [webhookId, eventType, targetUrl, 'pending', attempt, maxAttempts, JSON.stringify(payload)]
  );
  return rows[0].id;
}

async function updateDeliveryLog(id, status, statusCode, responseTime, error) {
  await db.query(
    `UPDATE webhook_deliveries
     SET status = $1, response_status = $2, response_time_ms = $3, error = $4, delivered_at = NOW()
     WHERE id = $5`,
    [status, statusCode, responseTime, error, id]
  );
}

async function deliverWebhook(webhookId, url, secret, payload, attempt = 0) {
  const ssrfCheck = await validateOutboundUrl(url);
  if (!ssrfCheck.valid) {
    logger.error('Webhook delivery blocked: URL failed SSRF validation', { url, reason: ssrfCheck.error });
    await createDeliveryLog(webhookId, payload.event, url, attempt + 1, MAX_ATTEMPTS, payload)
      .then((id) => updateDeliveryLog(id, 'failed', null, null, 'SSRF validation failed'));
    return;
  }
  const body = JSON.stringify(payload);
  const signature = sign(secret, body);
  const deliveryId = await createDeliveryLog(webhookId, payload.event, url, attempt + 1, MAX_ATTEMPTS, payload);
  const start = Date.now();
  try {
    const statusCode = await httpsPost(url, body, signature, ssrfCheck.agent);
    const responseTime = Date.now() - start;
    await updateDeliveryLog(deliveryId, 'delivered', statusCode, responseTime, null);
  } catch (err) {
    const responseTime = Date.now() - start;
    const statusCodeMatch = err.message.match(/HTTP (\d+)/);
    const statusCode = statusCodeMatch ? parseInt(statusCodeMatch[1]) : null;
    if (attempt < MAX_ATTEMPTS - 1) {
      const delay = Math.pow(2, attempt) * 1000;
      setTimeout(() => deliverWebhook(webhookId, url, secret, payload, attempt + 1), delay);
    } else {
      await updateDeliveryLog(deliveryId, 'failed', statusCode, responseTime, err.message);
    }
  }
}

module.exports = { deliverWebhook };
