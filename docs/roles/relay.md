# Role: Relay

A small **operator-run** server (on our VPS) that helps peers connect. Not a seeder, not trusted
with content.

## The problem it solves

Both a home seeder and a home downloader sit behind NAT routers. iroh tries to **hole-punch** a
direct peer-to-peer connection (~90% success). When that fails (~5-10% - strict/symmetric NATs,
UDP-hostile networks), the two peers need a middleman to forward packets. That's the relay.

## What it does (and doesn't)

- **Does:** forward traffic between two peers that couldn't connect directly, so the transfer
  still happens.
- **Doesn't:** store models, verify anything, or decide truth. It moves encrypted QUIC packets
  it can't read.

## Why self-host it on the VPS

- The free n0 public relays are shared, rate-limited, and only guaranteed through end-2026.
- The relay binary is open-source - run our own on the VPS we already have.
- **Cost is bounded:** the relay only carries the minority of connections that can't go direct;
  the bulk of every transfer flows peer-to-peer and never touches it.

## Mental model

The relay is the **emergency lane**, not the highway. Most cars (bytes) take the direct road
(hole-punched P2P); a few that can't merge use the relay.

See: [Seeder](seeder.md) · [Client](client.md).
