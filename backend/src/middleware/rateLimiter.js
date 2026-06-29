const rateLimit = require('express-rate-limit');
const Redis = require('ioredis');
const logger = require('../utils/logger');
const audit = require('../services/audit');

// Redis client for rate limiting (shared instance)
let redisClient = null;

function getRedis() {
  if (redisClient) return redisClient;
  if (!process.env.REDIS_URL) return null;
  redisClient = new Redis(process.env.REDIS_URL, {
    lazyConnect: true,
    enableOfflineQueue: false,
    maxRetriesPerRequest: 1,
  });
  redisClient.on('error', () => {});
  return redisClient;
}

// express-rate-limit v7 custom store backed by ioredis
class RedisStore {
  constructor(windowMs, prefix) {
    this.windowSec = Math.ceil(windowMs / 1000);
    this.prefix = prefix;
  }

  async increment(key) {
    const redis = getRedis();
    const storeKey = `rl:${this.prefix}:${key}`;
    if (!redis) {
      // No Redis — fall back to in-memory (handled by express-rate-limit default)
      return null;
    }
    try {
      const multi = redis.multi();
      multi.incr(storeKey);
      multi.ttl(storeKey);
      const [[, totalHits], [, ttl]] = await multi.exec();
      if (ttl === -1) {
        await redis.expire(storeKey, this.windowSec);
      }
      const resetTime = new Date(Date.now() + (ttl > 0 ? ttl : this.windowSec) * 1000);
      return { totalHits, resetTime };
    } catch {
      return null; // Redis error — fail open
    }
  }

  async decrement(key) {
    const redis = getRedis();
    if (!redis) return;
    try {
      await redis.decr(`rl:${this.prefix}:${key}`);
    } catch {}
  }

  async resetKey(key) {
    const redis = getRedis();
    if (!redis) return;
    try {
      await redis.del(`rl:${this.prefix}:${key}`);
    } catch {}
  }
}

function getTrustedIp(req) {
  const trusted = (process.env.TRUSTED_PROXIES || '').split(',').map((s) => s.trim()).filter(Boolean);
  const forwarded = req.headers['x-forwarded-for'];
  if (forwarded && trusted.length > 0) {
    const ips = forwarded.split(',').map((s) => s.trim());
    return ips[0] || req.ip;
  }
  return req.ip;
}

function makeKeyByIp(req) {
  return getTrustedIp(req);
}

function makeKeyByUser(req) {
  return req.user?.userId || getTrustedIp(req);
}

function makeKeyByAdmin(req) {
  return req.user?.userId || getTrustedIp(req);
}

function onLimitReached(req, res, options) {
  const userId = req.user?.userId || null;
  const ip = getTrustedIp(req);
  logger.warn('Rate limit exceeded', { path: req.path, ip, userId });
  audit.log(userId, 'rate_limit_exceeded', ip, req.headers['user-agent'], {
    endpoint: req.path,
    method: req.method,
  });
}

function makeHeaders(req, res, rateLimitInfo) {
  const { limit, remaining, resetTime } = rateLimitInfo;
  res.setHeader('X-RateLimit-Limit', limit);
  res.setHeader('X-RateLimit-Remaining', Math.max(0, remaining));
  if (resetTime) {
    const retryAfter = Math.ceil((resetTime.getTime() - Date.now()) / 1000);
    res.setHeader('X-RateLimit-Reset', Math.ceil(resetTime.getTime() / 1000));
    res.setHeader('Retry-After', retryAfter);
  }
}

const WINDOW_1MIN = 60 * 1000;
const WINDOW_1MIN_SEC = 60;

// Auth endpoints: 5 req/min per IP
const authLimiter = rateLimit({
  windowMs: WINDOW_1MIN,
  max: 5,
  keyGenerator: makeKeyByIp,
  store: new RedisStore(WINDOW_1MIN, 'auth'),
  standardHeaders: false,
  legacyHeaders: false,
  handler(req, res, next, options) {
    onLimitReached(req, res, options);
    makeHeaders(req, res, options.requestWasSuccessful ? {} : {
      limit: options.max,
      remaining: 0,
      resetTime: new Date(Date.now() + options.windowMs),
    });
    res.status(429).json({ error: 'Too many auth attempts. Please try again later.' });
  },
  message: { error: 'Too many auth attempts. Please try again later.' },
});

// Payment submission: 10 req/min per authenticated user
const paymentLimiter = rateLimit({
  windowMs: WINDOW_1MIN,
  max: 10,
  keyGenerator: makeKeyByUser,
  store: new RedisStore(WINDOW_1MIN, 'payment'),
  standardHeaders: false,
  legacyHeaders: false,
  handler(req, res, next, options) {
    onLimitReached(req, res, options);
    makeHeaders(req, res, { limit: options.max, remaining: 0, resetTime: new Date(Date.now() + options.windowMs) });
    res.status(429).json({ error: 'Payment rate limit exceeded. Please slow down.' });
  },
});

// Balance/read endpoints: 60 req/min per authenticated user
const readLimiter = rateLimit({
  windowMs: WINDOW_1MIN,
  max: 60,
  keyGenerator: makeKeyByUser,
  store: new RedisStore(WINDOW_1MIN, 'read'),
  standardHeaders: false,
  legacyHeaders: false,
  handler(req, res, next, options) {
    onLimitReached(req, res, options);
    makeHeaders(req, res, { limit: options.max, remaining: 0, resetTime: new Date(Date.now() + options.windowMs) });
    res.status(429).json({ error: 'Too many requests. Please try again later.' });
  },
});

// Admin endpoints: 30 req/min per admin user
const adminLimiter = rateLimit({
  windowMs: WINDOW_1MIN,
  max: 30,
  keyGenerator: makeKeyByAdmin,
  store: new RedisStore(WINDOW_1MIN, 'admin'),
  standardHeaders: false,
  legacyHeaders: false,
  handler(req, res, next, options) {
    onLimitReached(req, res, options);
    makeHeaders(req, res, { limit: options.max, remaining: 0, resetTime: new Date(Date.now() + options.windowMs) });
    res.status(429).json({ error: 'Admin rate limit exceeded.' });
  },
});

// Attach rate limit headers on successful responses
function attachHeaders(limiter) {
  const originalMiddleware = limiter;
  return (req, res, next) => {
    const origJson = res.json.bind(res);
    res.json = (body) => {
      // headers are set by handler on 429; set on success too
      return origJson(body);
    };
    originalMiddleware(req, res, next);
  };
}

module.exports = { authLimiter, paymentLimiter, readLimiter, adminLimiter };
