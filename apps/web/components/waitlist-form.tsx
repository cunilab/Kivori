'use client';

import Link from 'next/link';
import { useEffect, useId, useRef, useState, type FormEvent, type ReactElement } from 'react';
import { Button } from '@kivori/ui/components/button';
import { Input } from '@kivori/ui/components/input';
import { Label } from '@kivori/ui/components/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@kivori/ui/components/select';
import { Textarea } from '@kivori/ui/components/textarea';
import { HONEYPOT_FIELD, LIMITS } from '@/lib/waitlist';

export interface ProductOption {
  slug: string;
  name: string;
}

interface WaitlistFormProps {
  /** Turnstile site key, read from the Worker env at request time by the server component. */
  siteKey: string;
  products: ProductOption[];
  /** Pre-selected product slug (`?product=`, or the product page); unknown values fall back. */
  initialProduct?: string | undefined;
  /** Prefix for element ids, so two forms on one page never clash. */
  idPrefix?: string | undefined;
}

interface TurnstileApi {
  render(
    container: HTMLElement,
    options: {
      sitekey: string;
      callback: (token: string) => void;
      'expired-callback': () => void;
      'error-callback': () => void;
    },
  ): string;
  reset(widgetId?: string): void;
}

declare global {
  interface Window {
    turnstile?: TurnstileApi;
  }
}

const SCRIPT_SRC = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit';

type Status = 'idle' | 'submitting' | 'success' | 'error';

function loadTurnstile(): Promise<TurnstileApi> {
  return new Promise((resolve, reject) => {
    if (window.turnstile) return resolve(window.turnstile);
    const done = (): void =>
      window.turnstile ? resolve(window.turnstile) : reject(new Error('turnstile'));
    const existing = document.querySelector<HTMLScriptElement>(`script[src="${SCRIPT_SRC}"]`);
    const script = existing ?? document.createElement('script');
    script.addEventListener('load', done);
    script.addEventListener('error', () => reject(new Error('turnstile')));
    if (!existing) {
      script.src = SCRIPT_SRC;
      script.async = true;
      document.head.appendChild(script);
    }
  });
}

export function WaitlistForm({
  siteKey,
  products,
  initialProduct,
  idPrefix = 'waitlist',
}: WaitlistFormProps): ReactElement {
  const uid = useId();
  const id = (name: string): string => `${idPrefix}${uid}-${name}`;
  const formRef = useRef<HTMLFormElement>(null);
  const widgetRef = useRef<HTMLDivElement>(null);
  const widgetId = useRef<string | undefined>(undefined);
  const [status, setStatus] = useState<Status>('idle');
  const [message, setMessage] = useState('');
  const [token, setToken] = useState('');
  const [attribution, setAttribution] = useState({
    utm_source: '',
    utm_medium: '',
    utm_campaign: '',
    referrer: '',
  });

  const productItems = [
    { value: 'general', label: 'Any / not sure yet' },
    ...products.map((product) => ({ value: product.slug, label: product.name })),
  ];
  const selected =
    initialProduct && products.some((product) => product.slug === initialProduct)
      ? initialProduct
      : 'general';

  // UTM parameters and the referrer are read in the browser (the page itself is not cached per URL).
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    setAttribution({
      utm_source: params.get('utm_source') ?? '',
      utm_medium: params.get('utm_medium') ?? '',
      utm_campaign: params.get('utm_campaign') ?? '',
      referrer: document.referrer,
    });
  }, []);

  // Load Turnstile lazily: when the form nears the viewport (or at once without IntersectionObserver).
  useEffect(() => {
    const form = formRef.current;
    let cancelled = false;
    const start = (): void => {
      loadTurnstile()
        .then((api) => {
          if (cancelled || !widgetRef.current || widgetId.current !== undefined) return;
          widgetId.current = api.render(widgetRef.current, {
            sitekey: siteKey,
            callback: setToken,
            'expired-callback': () => setToken(''),
            'error-callback': () => setToken(''),
          });
        })
        .catch(() => {
          if (!cancelled) {
            setStatus('error');
            setMessage('The human check could not load. Check your connection and reload.');
          }
        });
    };
    if (!form || typeof IntersectionObserver === 'undefined') {
      start();
      return () => {
        cancelled = true;
      };
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          observer.disconnect();
          start();
        }
      },
      { rootMargin: '300px' },
    );
    observer.observe(form);
    return () => {
      cancelled = true;
      observer.disconnect();
    };
  }, [siteKey]);

  async function onSubmit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (status === 'submitting') return;
    if (!token) {
      setStatus('error');
      setMessage('Please wait for the human check to finish, then send again.');
      return;
    }
    setStatus('submitting');
    setMessage('');
    const body: Record<string, string> = {};
    new FormData(event.currentTarget).forEach((value, key) => {
      if (typeof value === 'string') body[key] = value;
    });
    body.turnstileToken = token;
    try {
      const response = await fetch('/api/waitlist', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(body),
      });
      const data = (await response.json().catch(() => null)) as { message?: string } | null;
      if (response.ok) {
        setStatus('success');
        return;
      }
      setStatus('error');
      setMessage(data?.message ?? 'Something went wrong. Try again.');
    } catch {
      setStatus('error');
      setMessage('We could not reach the server. Check your connection and try again.');
    }
    // A Turnstile token works once, so a failed attempt needs a fresh one.
    setToken('');
    if (widgetId.current !== undefined) window.turnstile?.reset(widgetId.current);
  }

  if (status === 'success') {
    return (
      <div
        role="status"
        className="rounded-2xl border border-border bg-card p-6 text-left sm:text-center"
      >
        <p className="font-display text-2xl font-semibold">You&apos;re on the list</p>
        <p className="mt-2 text-muted-foreground">
          We will email you when there is news about Kivori. No spam, and you can ask us to delete
          your details any time.
        </p>
      </div>
    );
  }

  const submitting = status === 'submitting';

  return (
    <form
      ref={formRef}
      onSubmit={onSubmit}
      className="w-full rounded-2xl border border-border bg-card p-6 text-left"
    >
      <div>
        <Label htmlFor={id('email')}>Email</Label>
        <Input
          id={id('email')}
          name="email"
          type="email"
          required
          autoComplete="email"
          maxLength={LIMITS.email}
          className="mt-1.5 h-10"
        />
      </div>
      <div className="mt-4">
        <Label htmlFor={id('product')}>Which product?</Label>
        <Select name="product" defaultValue={selected} key={selected} items={productItems}>
          <SelectTrigger id={id('product')} className="mt-1.5 h-10 w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {productItems.map((item) => (
              <SelectItem key={item.value} value={item.value}>
                {item.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="mt-4">
        <Label htmlFor={id('use-case')}>
          What would you use Kivori for?{' '}
          <span className="font-normal text-muted-foreground">(optional)</span>
        </Label>
        <Textarea
          id={id('use-case')}
          name="use_case"
          rows={3}
          maxLength={LIMITS.useCase}
          className="mt-1.5"
        />
      </div>

      {/* Honeypot: hidden from people and from assistive tech; bots fill it in. */}
      <div aria-hidden="true" className="absolute -left-[9999px] h-0 w-0 overflow-hidden">
        <label htmlFor={id('website')}>Leave this field empty</label>
        <Input
          id={id('website')}
          name={HONEYPOT_FIELD}
          type="text"
          tabIndex={-1}
          autoComplete="off"
        />
      </div>
      <input type="hidden" name="utm_source" value={attribution.utm_source} />
      <input type="hidden" name="utm_medium" value={attribution.utm_medium} />
      <input type="hidden" name="utm_campaign" value={attribution.utm_campaign} />
      <input type="hidden" name="referrer" value={attribution.referrer} />

      <div ref={widgetRef} className="mt-4 min-h-[65px]" />

      <p
        role={status === 'error' ? 'alert' : undefined}
        className={`mt-2 min-h-5 text-sm ${status === 'error' ? 'font-medium text-destructive' : ''}`}
      >
        {status === 'error' ? message : ''}
      </p>

      <Button
        type="submit"
        size="lg"
        disabled={submitting}
        aria-busy={submitting}
        className="mt-2 h-10 w-full disabled:cursor-wait"
      >
        {submitting ? 'Sending...' : 'Join the waitlist'}
      </Button>
      <p className="mt-3 text-xs text-muted-foreground">
        We store your email, choice and answers only to tell you about Kivori. See the{' '}
        <Link href="/privacy" className="underline">
          privacy page
        </Link>
        .
      </p>
    </form>
  );
}
