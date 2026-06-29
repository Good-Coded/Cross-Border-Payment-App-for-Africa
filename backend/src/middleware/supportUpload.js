const multer = require('multer');
const path = require('path');
const fs = require('fs');
const { v4: uuidv4 } = require('uuid');
const logger = require('../utils/logger');

const ALLOWED_MIME_TYPES = ['image/jpeg', 'image/png', 'image/gif', 'application/pdf'];
const MAX_FILE_SIZE = 5 * 1024 * 1024; // 5 MB
const MAX_FILES = 3;

// Magic-byte signatures for supported MIME types
const MAGIC_SIGNATURES = {
  'image/jpeg': [[0xFF, 0xD8, 0xFF]],
  'image/png':  [[0x89, 0x50, 0x4E, 0x47]],
  'image/gif':  [[0x47, 0x49, 0x46, 0x38, 0x37, 0x61], [0x47, 0x49, 0x46, 0x38, 0x39, 0x61]],
  'application/pdf': [[0x25, 0x50, 0x44, 0x46]],
};

const uploadDir = path.resolve(__dirname, '../../../uploads/support');
if (!fs.existsSync(uploadDir)) {
  fs.mkdirSync(uploadDir, { recursive: true });
}

const MIME_EXTENSIONS = {
  'image/jpeg': '.jpg',
  'image/png': '.png',
  'image/gif': '.gif',
  'application/pdf': '.pdf',
};

const storage = multer.diskStorage({
  destination: uploadDir,
  filename: (_req, file, cb) => {
    const ext = MIME_EXTENSIONS[file.mimetype] || path.extname(file.originalname).toLowerCase();
    cb(null, `${uuidv4()}${ext}`);
  },
});

const fileFilter = (_req, file, cb) => {
  if (ALLOWED_MIME_TYPES.includes(file.mimetype)) {
    cb(null, true);
  } else {
    cb(Object.assign(
      new Error('Only JPEG, PNG, GIF, and PDF files are allowed'),
      { status: 400 }
    ));
  }
};

const upload = multer({
  storage,
  fileFilter,
  limits: { fileSize: MAX_FILE_SIZE, files: MAX_FILES },
}).array('attachments', MAX_FILES);

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

async function tryClamScan(filePath) {
  try {
    // eslint-disable-next-line import/no-extraneous-dependencies
    const NodeClam = require('clamscan');
    const scanner = await new NodeClam().init();
    const { isInfected } = await scanner.scanFile(filePath);
    return isInfected;
  } catch {
    logger.warn('ClamAV not configured — skipping malware scan', { filePath });
    return false;
  }
}

function cleanup(files) {
  if (!files) return;
  for (const f of files) {
    try { fs.unlinkSync(f.path); } catch {}
  }
}

async function supportUploadMiddleware(req, res, next) {
  upload(req, res, async (err) => {
    if (err) {
      cleanup(req.files);
      if (err.code === 'LIMIT_FILE_SIZE') {
        return res.status(400).json({ error: 'Each file must be 5 MB or smaller' });
      }
      if (err.code === 'LIMIT_FILE_COUNT') {
        return res.status(400).json({ error: 'Maximum 3 attachments allowed per ticket' });
      }
      return res.status(400).json({ error: err.message || 'File upload error' });
    }

    if (!req.files || req.files.length === 0) {
      return next();
    }

    for (const file of req.files) {
      if (!matchesMagicBytes(file.path, file.mimetype)) {
        cleanup(req.files);
        return res.status(400).json({
          error: `File "${file.originalname}" failed content validation — file content does not match declared type`,
        });
      }

      const isInfected = await tryClamScan(file.path);
      if (isInfected) {
        cleanup(req.files);
        return res.status(400).json({
          error: `File "${file.originalname}" was rejected by the malware scanner`,
        });
      }
    }

    next();
  });
}

module.exports = supportUploadMiddleware;
