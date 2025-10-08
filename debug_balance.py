#!/usr/bin/env python3
"""
Debug script to see raw balance response
"""

import json
import sys
import os
import green_gdk as gdk

# Suppress warnings
original_stderr = sys.stderr
sys.stderr = open(os.devnull, 'w')

try:
    gdk.init({'datadir': '.', 'log_level': 'warn'})
except:
    pass

# AMP environment mnemonic
mnemonic = "perfect grit monkey disorder cliff exhibit meat giant dream secret liberty walnut later caught snow wasp bulb bind feature couple vault flower credit bundle"

# Create session
session = gdk.Session({'name': 'testnet-liquid'})
credentials = {'mnemonic': mnemonic}

try:
    session.login_user({}, credentials).resolve()
except:
    session.register_user({}, credentials).resolve()
    session.login_user({}, credentials).resolve()

# Find GA44YYwPM8vuRMmjFL8i5kSqXhoTW2
subaccounts_result = session.get_subaccounts().resolve()
subaccount_pointer = None

for subaccount in subaccounts_result['subaccounts']:
    if subaccount['receiving_id'] == 'GA44YYwPM8vuRMmjFL8i5kSqXhoTW2':
        subaccount_pointer = subaccount['pointer']
        break

if subaccount_pointer:
    print(f"Found GAID at pointer {subaccount_pointer}")
    
    # Get balance with num_confs=0
    print("\nRaw balance response:")
    balance = session.get_balance({
        'subaccount': subaccount_pointer,
        'num_confs': 0
    }).resolve()
    
    print(json.dumps(balance, indent=2))
    
    # Also try get_balance without subaccount filter
    print("\nWallet-wide balance:")
    wallet_balance = session.get_balance({'num_confs': 0}).resolve()
    print(json.dumps(wallet_balance, indent=2))
    
    # Get unspent outputs
    print("\nUnspent outputs:")
    try:
        unspent = session.get_unspent_outputs({
            'subaccount': subaccount_pointer,
            'num_confs': 0
        }).resolve()
        print(f"Number of unspent outputs: {len(unspent.get('unspent_outputs', {}))}")
        for k, v in unspent.get('unspent_outputs', {}).items():
            print(f"  {k}: {v.get('satoshi')} satoshi")
    except Exception as e:
        print(f"Error: {e}")

# Restore stderr
sys.stderr.close()
sys.stderr = original_stderr