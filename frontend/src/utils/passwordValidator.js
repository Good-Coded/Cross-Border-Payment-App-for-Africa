/**
 * Password validation utility matching backend rules
 * Rules: min 8 chars, uppercase, lowercase, digit, special character
 */

export function getPasswordStrength(password) {
  const checks = {
    length: password.length >= 8,
    uppercase: /[A-Z]/.test(password),
    lowercase: /[a-z]/.test(password),
    number: /[0-9]/.test(password),
    special: /[^A-Za-z0-9]/.test(password),
  };

  const score = Object.values(checks).filter(Boolean).length;
  const levels = ['', 'weak', 'fair', 'strong', 'very strong'];
  const colors = ['', 'bg-red-500', 'bg-orange-500', 'bg-yellow-500', 'bg-green-500'];
  const textColors = ['', 'text-red-500', 'text-orange-500', 'text-yellow-500', 'text-green-500'];

  return {
    checks,
    score,
    label: levels[score],
    barColor: colors[score],
    textColor: textColors[score],
    isValid: score >= 4, // Must meet all 5 requirements
  };
}

export function validateEmail(email) {
  const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
  return emailRegex.test(email);
}

export function getEmailError(email) {
  if (!email) return 'Email is required';
  if (!validateEmail(email)) return 'Please enter a valid email address';
  return '';
}

export function getPasswordError(password) {
  if (!password) return 'Password is required';
  if (password.length < 8) return 'Password must be at least 8 characters';

  const strength = getPasswordStrength(password);
  const unmet = [];

  if (!strength.checks.uppercase) unmet.push('uppercase letter');
  if (!strength.checks.lowercase) unmet.push('lowercase letter');
  if (!strength.checks.number) unmet.push('number');
  if (!strength.checks.special) unmet.push('special character');

  if (unmet.length > 0) {
    return `Password must contain at least one ${unmet.join(', ')}`;
  }

  return '';
}
