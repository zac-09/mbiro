# Licence service

Activates licence keys, issues them when someone buys, and lists refunded ones
(`PLAN.md` section 8). The app's side is `apps/desktop/src-tauri/src/activation.rs`.

| Request                        | Does                                                             |
| ------------------------------ | ---------------------------------------------------------------- |
| `POST /v1/activate`            | Gives a Mac one of a key's three places; replies with a receipt. |
| `POST /v1/deactivate`          | Frees that place.                                                |
| `GET /v1/revoked`              | Lists the ids of refunded licences.                              |
| `POST /v1/payments/<provider>` | A payment provider's webhook: a purchase or a refund.            |
| `GET /activate`                | The page behind the activation link in the licence email.        |

A licence key and a receipt are both `<payload>.<signature>`, signed with the
Ed25519 private key that `scripts/licence-keygen.sh` makes. The app holds the
public half and checks both without the network. A licence's id is the SHA-256
of its key; the service stores ids and hashed device codes, never a hardware
identifier.

## What is here and what is not

`src/service.ts` is the whole service as one function from a request to a
reply. It runs anywhere that can hand it a `Request`, and takes three things
that depend on decisions not made yet:

- **A store** (`src/store.ts`). Only the in-memory one exists. The hosted
  database needs an adapter with the same six methods, each one atomic.
- **A payment adapter** (`src/payments.ts`). Only the stand-in exists. The
  real provider needs an adapter that checks its webhook signature.
- **A mailer** (`src/mail.ts`). None exists; the local run prints the email.

Also not here: rate limiting (left to the host).

## The activation link

When the service is given its own address (`publicUrl`), the licence email
carries a link, `<publicUrl>/activate#<key>`. The page it opens passes the key
to the app as `mbiro://activate?key=<key>`, and the app activates it like a
pasted key. The email uses an ordinary https link because mail programs do not
reliably make a link with an app's own scheme clickable.

The key sits after the `#`, which browsers do not send to servers, so opening
the link does not show the key to this service or its logs. A Mac that already
has a licence ignores activation links.

## Run it locally

```sh
LICENCE_PRIVATE_KEY_FILE=/somewhere/safe/licence-private.pem \
STUB_PAYMENT_SECRET=<16 characters or more> \
pnpm --filter @mbiro/licence-service dev
```

It listens on `http://127.0.0.1:8787`, keeps everything in memory, and prints
the public key to paste into the app. Point a development build of the app at
it with `MBIRO_LICENCE_SERVER=http://127.0.0.1:8787 pnpm dev`.

## Test it

```sh
pnpm --filter @mbiro/licence-service test   # the service alone
scripts/test-licence-service.sh             # the app's client against it
```

Both use key pairs made for the run and thrown away.
