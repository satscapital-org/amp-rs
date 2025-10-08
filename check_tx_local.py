#!/usr/bin/env python3
"""
Script to check transaction details for a specific GAID.
"""

import json
import sys
import os
import green_gdk as gdk

def main():
    if len(sys.argv) < 3:
        print("Usage: python check_tx_local.py <environment> <gaid> [txid]")
        sys.exit(1)
    
    environment = sys.argv[1].lower()
    gaid = sys.argv[2]
    txid = sys.argv[3] if len(sys.argv) > 3 else None
    
    # Suppress GDK warnings
    original_stderr = sys.stderr
    sys.stderr = open(os.devnull, 'w')
    
    try:
        gdk.init({'datadir': '.', 'log_level': 'warn'})
    except:
        pass
    
    # Local mnemonics
    mnemonics = {
        'dev': "vote ball voice juice visit license music off paddle hold suffer beach someone hover wheat boring protect broccoli issue opinion uniform media update arrange",
        'test': "hood novel finish couch rack pistol march army gun bring universe unknown hurry famous vote enact inspire whisper violin blur chief into engage cigar",
        'amp': "perfect grit monkey disorder cliff exhibit meat giant dream secret liberty walnut later caught snow wasp bulb bind feature couple vault flower credit bundle"
    }
    
    mnemonic = mnemonics.get(environment)
    if not mnemonic:
        print(f"Invalid environment: {environment}")
        sys.exit(1)
    
    # Create session
    session = gdk.Session({'name': 'testnet-liquid'})
    credentials = {'mnemonic': mnemonic}
    
    try:
        session.login_user({}, credentials).resolve()
    except:
        session.register_user({}, credentials).resolve()
        session.login_user({}, credentials).resolve()
    
    # Find the subaccount
    subaccounts_result = session.get_subaccounts().resolve()
    subaccount_pointer = None
    
    for subaccount in subaccounts_result['subaccounts']:
        if subaccount['receiving_id'] == gaid and subaccount['type'] == '2of2_no_recovery':
            subaccount_pointer = subaccount['pointer']
            break
    
    if subaccount_pointer is None:
        print(f"GAID {gaid} not found")
        sys.exit(1)
    
    print(f"Found GAID {gaid} at pointer {subaccount_pointer}")
    
    # Get balance with different confirmation levels
    print("\nBalance Information:")
    for num_confs in [0, 1]:
        try:
            balance = session.get_balance({
                'subaccount': subaccount_pointer,
                'num_confs': num_confs
            }).resolve()
            
            total = 0
            for asset_id, info in balance.items():
                if isinstance(info, dict) and 'satoshi' in info:
                    total += info['satoshi']
                    print(f"  Asset {asset_id}: {info['satoshi']} satoshi")
            
            print(f"  Total balance with {num_confs} confirmations: {total} satoshi")
        except Exception as e:
            print(f"  Error getting balance with {num_confs} confs: {e}")
    
    # Get unspent outputs
    try:
        unspent = session.get_unspent_outputs({
            'subaccount': subaccount_pointer,
            'num_confs': 0
        }).resolve()
        
        print(f"\nUnspent outputs: {len(unspent.get('unspent_outputs', {}))}")
        for utxo_id, utxo in list(unspent.get('unspent_outputs', {}).items())[:10]:
            print(f"  - {utxo_id}: {utxo.get('satoshi', 0)} sat (block: {utxo.get('block_height', 'unconfirmed')})")
            
    except Exception as e:
        print(f"Error getting unspent outputs: {e}")
    
    # Get transaction list
    print("\nTransaction History:")
    try:
        # Get transactions for this subaccount
        tx_list = session.get_transactions({
            'subaccount': subaccount_pointer,
            'first': 0,
            'count': 100,
            'num_confs': 0  # Include unconfirmed
        }).resolve()
        
        print(f"Found {len(tx_list.get('transactions', []))} transactions")
        
        if txid:
            print(f"\nLooking for transaction: {txid}")
            found_tx = False
            for tx in tx_list.get('transactions', []):
                if tx.get('txhash') == txid:
                    found_tx = True
                    print(f"\n✓ Transaction found!")
                    print(f"  Type: {tx.get('type')}")
                    print(f"  Amount: {tx.get('satoshi')} satoshi")
                    print(f"  Fee: {tx.get('fee')} satoshi")
                    print(f"  Block Height: {tx.get('block_height', 'unconfirmed')}")
                    print(f"  Confirmations: {tx.get('confirmations', 0)}")
                    print(f"  Created at: {tx.get('created_at_ts', 'unknown')}")
                    print(f"  Memo: {tx.get('memo', 'none')}")
                    break
            
            if not found_tx:
                print(f"\n✗ Transaction {txid} NOT found in this subaccount")
        
        # Show recent transactions
        print("\nRecent transactions (last 10):")
        for i, tx in enumerate(tx_list.get('transactions', [])[:10]):
            print(f"  {i+1}. {tx.get('txhash', 'unknown')[:16]}...")
            print(f"     Type: {tx.get('type', 'unknown')}, Amount: {tx.get('satoshi', 0)} sat")
            print(f"     Block: {tx.get('block_height', 'unconfirmed')}, Confirmations: {tx.get('confirmations', 0)}")
            
    except Exception as e:
        print(f"Error getting transactions: {e}")
    
    # Get and display some addresses to verify
    print("\nAddress verification:")
    try:
        # Current address
        current_addr = session.get_receive_address({'subaccount': subaccount_pointer}).resolve()
        print(f"Current receive address: {current_addr['address']}")
        
        # Previous addresses
        prev_addrs = session.get_previous_addresses({
            'subaccount': subaccount_pointer,
            'last_pointer': 10
        }).resolve()
        
        print("Previous addresses:")
        for i, addr_info in enumerate(prev_addrs.get('list', [])[:5]):
            print(f"  {i}: {addr_info.get('address', 'N/A')}")
            
    except Exception as e:
        print(f"Error getting addresses: {e}")
    
    # Restore stderr
    sys.stderr.close()
    sys.stderr = original_stderr

if __name__ == "__main__":
    main()