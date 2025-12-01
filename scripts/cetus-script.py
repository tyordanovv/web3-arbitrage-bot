import json
import os
import re

with open("cetus-pools.json", "r") as f:
    data = json.load(f)

pools = data.get("data", {}).get("list", [])

# ==========================================
# TOKENS.TXT (merge without overwriting)
# ==========================================
tokens_set = set()

# Load existing token lines if file exists
if os.path.exists("tokens.txt"):
    with open("tokens.txt", "r") as f:
        for line in f:
            line = line.strip()
            if " = " in line:
                symbol, address = line.split(" = ")
                tokens_set.add((symbol.strip(), address.strip()))

# Add new tokens
for pool in pools:
    tokens_set.add((pool["coinA"]["symbol"], pool["coinA"]["coinType"]))
    tokens_set.add((pool["coinB"]["symbol"], pool["coinB"]["coinType"]))

# Write merged tokens back
with open("tokens.txt", "w") as f:
    for symbol, address in sorted(tokens_set):
        f.write(f"{symbol} = {address}\n")


# ==========================================
# POOLS.TOML (merge pool blocks only)
# ==========================================

# Header block stays unchanged
toml_header = '''[network]
network = "SuiMainnet"
rpc_url = "https://fullnode.mainnet.sui.io:443"
ws_url = "wss://fullnode.mainnet.sui.io:443"

[[network.dexes]]
id = "Cetus"
package_id = "0x686e66a7a993b58e3e5c0f633c0541d1a67b8b81c6728bfc53b317c355d4d2e0"
event_type = "SwapEvent"
enabled = true
'''

existing_pools = set()
existing_content = ""

# Load existing pools.toml
if os.path.exists("cetus-pools.toml"):
    with open("cetus-pools.toml", "r") as f:
        existing_content = f.read()

    # Extract pool addresses from existing pool blocks
    pool_regex = r'address\s*=\s*"([^"]+)"'
    existing_pools = set(re.findall(pool_regex, existing_content))


# Build merged TOML
final_toml = toml_header
merged_blocks = set()

# Reconstruct existing blocks after the header
if existing_content:
    # Keep everything after the header
    parts = existing_content.split('[[network.dexes.pools]]')
    if len(parts) > 1:
        for block in parts[1:]:
            block = '[[network.dexes.pools]]' + block
            # extract address and store block
            m = re.search(r'address\s*=\s*"([^"]+)"', block)
            if m:
                addr = m.group(1)
                merged_blocks.add((addr, block))

# Add missing new pools
for pool in pools:
    pool_address = pool["pool"]

    if pool_address in existing_pools:
        continue  # already present, skip

    token_a = pool["coinA"]
    token_b = pool["coinB"]

    block = f'''
[[network.dexes.pools]]
address = "{pool_address}"
token_a = {{ symbol = "{token_a["symbol"]}", decimals = {token_a["decimals"]} }}
token_b = {{ symbol = "{token_b["symbol"]}", decimals = {token_b["decimals"]} }}
'''
    merged_blocks.add((pool_address, block))

# Write final file
for _, block in sorted(merged_blocks):
    final_toml += block

with open("cetus-pools.toml", "w") as f:
    f.write(final_toml)

print("tokens.txt and pools.toml merged successfully.")
