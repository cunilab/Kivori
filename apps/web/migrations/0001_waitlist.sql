-- Waitlist sign-ups. No IP address or user agent is stored.
CREATE TABLE IF NOT EXISTS waitlist (
  id INTEGER PRIMARY KEY,
  email TEXT NOT NULL,
  product TEXT NOT NULL,
  use_case TEXT,
  utm_source TEXT,
  utm_medium TEXT,
  utm_campaign TEXT,
  referrer TEXT,
  created_at TEXT NOT NULL,
  UNIQUE (email, product)
);
