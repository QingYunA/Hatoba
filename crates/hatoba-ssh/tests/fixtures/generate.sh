#!/usr/bin/env bash
# Regenerates the TEST-ONLY key fixtures in keys/. None of these keys protect anything.
# Requires: ssh-keygen (OpenSSH 9.x), openssl, puttygen (>= 0.78 for ppk v3).
# Passphrase of every encrypted fixture: hatoba-test
set -euo pipefail
cd "$(dirname "$0")"
rm -rf keys && mkdir keys && cd keys
PASS=hatoba-test
echo -n "$PASS" > pass.txt

# OpenSSH format
ssh-keygen -q -t ed25519 -N '' -C 'test-ed25519' -f ed25519
ssh-keygen -q -t ed25519 -N "$PASS" -C 'test-ed25519-enc' -f ed25519_enc
ssh-keygen -q -t rsa -b 2048 -N '' -C 'test-rsa' -f rsa
ssh-keygen -q -t rsa -b 2048 -N "$PASS" -C 'test-rsa-enc' -f rsa_enc
ssh-keygen -q -t ecdsa -b 256 -N '' -C 'test-ecdsa256' -f ecdsa256
ssh-keygen -q -t ecdsa -b 384 -N '' -C 'test-ecdsa384' -f ecdsa384
ssh-keygen -q -t ecdsa -b 521 -N '' -C 'test-ecdsa521' -f ecdsa521
ssh-keygen -q -t ecdsa -b 256 -N "$PASS" -C 'test-ecdsa256-enc' -f ecdsa256_enc

# PEM: PKCS#1 RSA, SEC1 EC, PKCS#8 (plain and encrypted)
ssh-keygen -q -t rsa -b 2048 -m PEM -N '' -C pem -f rsa_pkcs1
ssh-keygen -q -t rsa -b 2048 -m PEM -N "$PASS" -C pem -f rsa_pkcs1_enc
ssh-keygen -q -t ecdsa -b 256 -m PEM -N '' -C pem -f ec_sec1
ssh-keygen -q -t ecdsa -b 256 -m PEM -N "$PASS" -C pem -f ec_sec1_enc
# ssh-keygen ignores -m PKCS8 for ed25519, so use openssl for real PKCS#8 ed25519 keys
openssl genpkey -algorithm ed25519 -out ed25519_pkcs8 2>/dev/null
# ssh-keygen cannot read PKCS#8 ed25519, so build the .pub line from the raw public key
openssl pkey -in ed25519_pkcs8 -pubout -outform DER 2>/dev/null | tail -c 32 | python3 -c '
import base64, struct, sys
raw = sys.stdin.buffer.read()
blob = struct.pack(">I", 11) + b"ssh-ed25519" + struct.pack(">I", len(raw)) + raw
print("ssh-ed25519", base64.b64encode(blob).decode(), "pkcs8")' > ed25519_pkcs8.pub
ssh-keygen -q -t rsa -b 2048 -m PKCS8 -N '' -C pem -f rsa_pkcs8
ssh-keygen -q -t ecdsa -b 256 -m PKCS8 -N '' -C pem -f ec_pkcs8
openssl pkcs8 -topk8 -v2 aes-256-cbc -in ed25519_pkcs8 -passout pass:"$PASS" -out ed25519_pkcs8_enc 2>/dev/null
cp ed25519_pkcs8.pub ed25519_pkcs8_enc.pub
ssh-keygen -q -t rsa -b 2048 -m PKCS8 -N "$PASS" -C pem -f rsa_pkcs8_enc
# Legacy encrypted PEM with AES-256-CBC (ssh-keygen only writes AES-128)
openssl rsa -in rsa_pkcs1 -aes256 -traditional -passout pass:"$PASS" -out rsa_pkcs1_aes256 2>/dev/null

# PuTTY: v2 and v3, plain and encrypted, converted from the OpenSSH keys above
for v in 2 3; do
  puttygen ed25519 -O private -o ed25519_v$v.ppk --ppk-param version=$v
  puttygen ed25519 -O private -o ed25519_v${v}_enc.ppk --ppk-param version=$v --new-passphrase pass.txt
  puttygen rsa -O private -o rsa_v$v.ppk --ppk-param version=$v
  puttygen rsa -O private -o rsa_v${v}_enc.ppk --ppk-param version=$v --new-passphrase pass.txt
  puttygen ecdsa256 -O private -o ecdsa256_v$v.ppk --ppk-param version=$v
  puttygen ecdsa384 -O private -o ecdsa384_v$v.ppk --ppk-param version=$v
  puttygen ecdsa521 -O private -o ecdsa521_v$v.ppk --ppk-param version=$v
  puttygen ecdsa256 -O private -o ecdsa256_v${v}_enc.ppk --ppk-param version=$v --new-passphrase pass.txt
done

# Expected fingerprints ("<key name> <SHA256:...>"), straight from OpenSSH
for f in *.pub; do echo "${f%.pub} $(ssh-keygen -l -E sha256 -f "$f" | awk '{print $2}')"; done > fingerprints.txt
