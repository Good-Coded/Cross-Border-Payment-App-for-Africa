const multer = require('multer');
const path = require('path');
const fs = require('fs');
const { v4: uuidv4 } = require('uuid');
const logger = require('../utils/logger');

const ALLOWED_MIME_TYPES = ['image/jpeg', 'image/png', 'image/gif', 'application/pdf'];
const MAX_FILE_SIZE = 10 * 1024 * 1024; // 10 MB

const MAGIC_SIGNATURES = {
  'image/jpeg': [[0xFF, 0xD8, 0xFF]],
  'image/png': [[0x89, 0x50, 0x4E, 0x47]],
  'image/gif': [[0x47, 0x49, 0x46, 0x38, 0x37, 0x61], [0x47, 0x49, 0x46, 0x38, 0x39, 0x61]],
  'application/pdf': [[0x25, 0x50, 0x44, 0x46]],
};

const MIME_EXTENSIONS = {
  'image/jpeg': '.jpg',
  'image/png': '.png',
  'image/gif': '.gif',
  'application/pdf': '.pdf',
};

function getUploadDir() {
  const baseDir = path.resolve(__dirname, '../../../uploads/disputes');
  if (!fs.existsSync(baseDir)) {
    fs.mkdirSync(baseDir, { recursive: true });
  }
  return baseDir;
}

function createStorage(disputeId) {
  const disputeDir = path.join(getUploadDir(), disputeId);
  if (!fs.existsSync(disputeDir)) {
    fs.mkdirSync(disputeDir, { recursive: true });
  }
  return multer.diskStorage({
    destination: disputeDir,
    filename: (_req, file, cb) => {
      const ext = MIME_EXTENSIONS[file.mimetype] || path.extname(file.originalname).toLowerCase();
      cb(null, `${uuidv4()}${ext}`);
    },
  });
}

function getUploader(disputeId) {
  const storage = createStorage(disputeId);
  return multer({
    storage,
    fileFilter: (_req, file, cb) => {
      if (ALLOWED_MIME_TYPES.includes(file.mimetype)) {
        cb(null, true);
      } else {
        cb(Object.assign(
          new Error('Only JPEG, PNG, GIF, and PDF files are allowed'),
          { status: 400 }
        ));
      }
    },
    limits: { fileSize: MAX_FILE_SIZE },
  }).single('file');
}

function matchesMagicBytes(filePath, mimeType) {
  const signatures = MAGIC_SIGNATURES[mimeType];
  if (!signatures) return false;

  const maxLen = Math.max(...signatures.map(s => s.length));
  const buf = Buffer.alloc(maxLen);
  const fd = fs.openSync(filePath, 'r');
  try {
    fs.readSync(fd, buf, 0, maxLen, 0);
  } finally {
    fs.closeSync(fd);
  }

  return signatures.some(sig =>
    sig.every((byte, i) => buf[i] === byte)
  );
}

async function computeSha256(filePath) {
  const crypto = require('crypto');
  const hash = crypto.createHash('sha256');
  const stream = fs.createReadStream(filePath);
  return new Promise((resolve, reject) => {
    stream.pipe(hash).on('finish', () => resolve(hash.read().toString('hex')));
    stream.on('error', reject);
  });
}

async function tryClamScan(filePath) {
  try {
    const NodeClam = require('clamscan');
    const scanner = await new NodeClam().init();
    const { isInfected } = await scanner.scanFile(filePath);
    return isInfected;
  } catch {
    logger.warn('ClamAV not configured — skipping malware scan', { filePath });
    return false;
  }
}

function disputeEvidenceMiddleware(req, res, next) {
  const disputeId = req.params.id;
  const upload = getUploader(disputeId);

  upload(req, res, async (err) => {
    if (err) {
      if (req.file) {
        try { fs.unlinkSync(req.file.path); } catch {}
      }
      if (err.code === 'LIMIT_FILE_SIZE') {
        return res.status(400).json({ error: 'Each file must be 10 MB or smaller' });
      }
      return res.status(400).json({ error: err.message || 'File upload error' });
    }

    if (!req.file) {
      return next();
    }

    if (!matchesMagicBytes(req.file.path, req.file.mimetype)) {
      try { fs.unlinkSync(req.file.path); } catch {}
      return res.status(400).json({
        error: `File "${req.file.originalname}" failed content validation — file content does not match declared type`,
      });
    }

    const isInfected = await tryClamScan(req.file.path);
    if (isInfected) {
      try { fs.unlinkSync(req.file.path); } catch {}
      return res.status(400).json({
        error: `File "${req.file.originalname}" was rejected by the malware scanner`,
      });
    }

    req.evidenceFile = {
      path: req.file.path,
      originalName: req.file.originalname,
      sha256Promise: computeSha256(req.file.path),
    };

    next();
  });
}

module.exports = { disputeEvidenceMiddleware, computeSha256 };