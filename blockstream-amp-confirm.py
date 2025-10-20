#!/usr/bin/env pyhton3

import argparse
import json
import logging
import re
import requests
import time
import sys


NODE_NAME = 'Elements Core'
MIN_SUPPORTED_ELEMENTS_VERSION = 170001  # 0.17.0.1
CLIENT_SCRIPT_VERSION = 2  # 0.0.2
COMMANDS = ['reissue', 'distribute', 'burn', 'update-blinders']


# adapted from https://github.com/Blockstream/liquid_multisig_issuance
class RPCHost(object):

    def __init__(self, url):
        self.session = requests.Session()
        if re.match(r'.*\.onion/*.*', url):
            self.session.proxies = {
                'http': 'socks5h://localhost:9050',
                'https': 'socks5h://localhost:9050',
            }
        self.url = url

    def call(self, rpc_method, *params):
        payload = json.dumps({"method": rpc_method, "params": list(params), "jsonrpc": "2.0"})
        connected = False
        max_tries = 5
        for tries in range(max_tries):
            try:
                response = self.session.post(self.url, headers={'content-type': 'application/json'}, data=payload)
                connected = True
                break
            except requests.exceptions.ConnectionError:
                time.sleep(10)

        if not connected:
            raise Exception('Failed to connect for remote procedure call.')

        if response.status_code not in (200, 500):
            raise Exception(f'RPC connection failure: {response.status_code} {response.reason}')

        response_json = response.json()
        if 'error' in response_json and response_json['error']:
            raise ValueError(json.dumps(response_json))
        return response_json['result']


def get_auth_headers(base_url, username, password):
    logging.debug('Obtaining token')
    url = base_url.format('user/obtain_token')
    headers = {'content-type': 'application/json'}
    payload = {'username': username, 'password': password}
    response = requests.post(url, data=json.dumps(payload), headers=headers)
    assert response.status_code == 200
    token = json.loads(response.text).get('token')
    return {'content-type': 'application/json', 'Authorization': f'token {token}'}


def wait_for_confirmation(rpc, txid):
    # wait for 2 confirmations
    logging.warning(f'Transaction sent, waiting for transaction {txid} to be confirmed (expected 2 minutes)')
    step_sec = 15
    max_sec = 10 * 60  # FIXME: this may need to be increased
    for i in range(max_sec // step_sec):
        time.sleep(step_sec)
        if rpc.call('gettransaction', txid).get('confirmations', 0) > 1:
            return True
    logging.error(f'Transaction {txid} was not confirmed after {max_sec // 60} minutes')
    return False


def check_passphrase(rpc):
    # check passphrase presence with an invalid signmessage call, the error code allows us to recognize if a passphrase is needed
    try:
        rpc.call('signmessage', 'invalidaddress', 'message')
    except ValueError as e:
        error_json = json.loads(str(e))
        if error_json['error']['code'] == -13:
            logging.error(e)
            sys.exit(1)


def check_version(rpc):
    # TODO: log both to CLI (args.verbose) and to file (DEBUG)
    logging.debug(f'Script version: {CLIENT_SCRIPT_VERSION:06}')

    networkinfo = rpc.call('getnetworkinfo')
    node_version = networkinfo.get('version', 0)
    node_subversion = networkinfo.get('subversion', "")

    if NODE_NAME not in node_subversion:
        logging.error(f'Unexpected node ({node_subversion}), make sure you are connecting to a Elements node')
        sys.exit(1)

    if node_version < MIN_SUPPORTED_ELEMENTS_VERSION:
        logging.error(f'Node version ({node_version:06}) not supported (min: {MIN_SUPPORTED_ELEMENTS_VERSION:06})')
        sys.exit(1)

    logging.debug(f'Connected to Elements node, version: {node_version:06}')


def check_client_script(fj):
    min_supported_client_script_version = fj.get('min_supported_client_script_version', 0)
    if min_supported_client_script_version < CLIENT_SCRIPT_VERSION:
        logging.error(f'Client script version ({CLIENT_SCRIPT_VERSION:06}) not supported (min: {min_supported_client_script_version:06})')
        sys.exit(1)


def check_command(fj, command):
    # Check 'command' field in the json file
    script_command = fj.get('command')
    if script_command != command:
        logging.error(f'You have asked to perform a {command} but you have provided the wrong type of file for this action.')
        sys.exit(1)


def check_lost_output(base_url, headers, asset_uuid):
    # Wait for transactions propagation
    logging.info('Wait for 60 seconds ...')
    time.sleep(60)
    # Check lost outputs
    logging.debug('Check lost outputs.')
    balance_url = base_url.format(f'assets/{asset_uuid}/balance')
    response = requests.get(balance_url, headers=headers)

    if response.status_code != 200:
        logging.error('The Blockstream AMP API "balance" failed. '
                      'Transaction will not be sent.')
        sys.exit(1)

    if response.json()['lost_outputs'] != []:
        logging.error('The Blockstream AMP API "balance" returned some lost outputs. '
                      'Transaction will not be sent.')
        sys.exit(1)


def check_utxos(rpc, expected_utxos, expect_all=True):
    utxos = rpc.call('listunspent')
    local_utxos = [{'txid': x['txid'], 'vout': x['vout']} for x in utxos]
    num_found_utxos = sum(x in local_utxos for x in expected_utxos)
    if num_found_utxos == 0 or (expect_all and num_found_utxos != len(expected_utxos)):
        logging.error('Missing UTXO')
        sys.exit(1)


def check_assignments(base_url, headers, asset_uuid, distribution_uuid):
    # Check if distribution is confirmed searching in all assignments
    logging.debug('Check for confirmed distribution.')
    assignments_url = base_url.format(f'assets/{asset_uuid}/assignments')
    response = requests.get(assignments_url, headers=headers)

    if response.status_code != 200:
        logging.error('The Blockstream AMP API "assignments details" failed. '
                      'Distribution transaction will not be sent.')
        sys.exit(1)

    assignment_found = False
    for assignment in response.json():
        if assignment['distribution_uuid'] == distribution_uuid:
            assignment_found = True
            if assignment['is_distributed']:
                logging.error('This distribution has already been carried out and the transaction confirmed. '
                              'Distribution transaction will not be sent.')
                sys.exit(1)

    if not assignment_found:
        logging.error('The Blockstream AMP API "assignments details" did not included any assignment for the distribution uuid. '
                      'Distribution transaction will not be sent.')
        sys.exit(1)


def check_node_fully_synchronized(rpc):
    blockchaininfo = rpc.call('getblockchaininfo')
    progress = float(blockchaininfo.get('verificationprogress'))

    if progress < 0.999:
        logging.error('Your node is not fully synchronized, please wait until it is fully synchronized with the network')
        sys.exit(1)


def main():
    parser = argparse.ArgumentParser(
        description='Make transactions with the treasury node, and then confirm them to the Blockstream AMP Server.')

    parser.add_argument('-v', '--verbose', action='count', default=0, help='Be more verbose. Can be used multiple times.')

    parser.add_argument('-u', '--username', help='Blockstream AMP API username', required=True)
    parser.add_argument('-p', '--password', help='Blockstream AMP API password', required=True)
    parser.add_argument('-n', '--node-url', help='Elements node URL, eg http://USERNAME:PASSWORD@HOST:PORT/', required=True)

    subparsers = parser.add_subparsers(dest='command')
    subparsers.required = True
    for action in COMMANDS:
        action_parser = subparsers.add_parser(action)
        if action == 'update-blinders':
            action_parser.add_argument('-b', '--base-url', help='Blockstream AMP API url (default: https://amp.blockstream.com/api/{} or ' +
                                       'use https://amp-beta.blockstream.com/api/{} for beta platform)',
                                       default="https://amp.blockstream.com/api/{}")
            action_parser.add_argument('-a', '--asset-uuid', help='Blockstream AMP API asset uuid', required=True)
        else:
            action_parser.add_argument('-f', '--filename', type=argparse.FileType('r'),
                                       help=f'text file containing the output of the {action} request API calls', required=True)
            if action == 'reissue':
                action_parser.add_argument('--have-split-reissuance-token', action='store_true',
                                           help='Advanced option, use it only if requested by Blockstream AMP support')

            action_parser.add_argument('--use-existing', help='Skip transaction building and use an existing transaction, this flag requires ' +
                                       f'TXID{":VIN" if action == "reissue" else ""} of the existing transaction as argument.',
                                       default=argparse.SUPPRESS)

    args = parser.parse_args()

    if args.verbose == 0:
        logging.root.setLevel(logging.INFO)
    elif args.verbose > 0:
        logging.root.setLevel(logging.DEBUG)

    rpc = RPCHost(args.node_url)
    check_version(rpc)
    check_node_fully_synchronized(rpc)
    check_passphrase(rpc)

    if args.command != 'update-blinders':
        fj = json.load(args.filename)

        logging.debug(f'Opened file {args.filename}: {json.dumps(fj)}')

        check_client_script(fj)
        check_command(fj, args.command)

        base_url = fj.get('base_url')
        asset_uuid = fj.get('asset_uuid')
        asset_id = fj.get('asset_id')
    else:
        base_url = args.base_url
        asset_uuid = args.asset_uuid

    headers = get_auth_headers(base_url, args.username, args.password)

    if args.command != 'update-blinders':
        # The following checks are meant to mitigate the chance to run the script
        # improperly, which may lead to undesired outcomes, or eventually
        # non-recoverable states.

        check_lost_output(base_url, headers, asset_uuid)

    if args.command == 'reissue':
        # reissuance specific checks
        amount = fj.get('amount')
        reissuance_utxos = fj.get('reissuance_utxos')
        if 'use_existing' not in args:
            check_utxos(rpc, reissuance_utxos, expect_all=(not args.have_split_reissuance_token))

            # call the reissueasset on the node and wait for confirmation
            reissuance_output = rpc.call('reissueasset', asset_id, amount)
            logging.info(f'Reissuance transaction {reissuance_output["txid"]} vin {reissuance_output["vin"]}.')
            found = wait_for_confirmation(rpc, reissuance_output['txid'])
            if not found:
                sys.exit(1)
            txid = reissuance_output['txid']
        else:
            try:
                txid, vin = args.use_existing.split(':')
                vin = int(vin)
            except ValueError:
                logging.error('If you use "--use-existing" argument you will also need to use TXID:VIN.')
                sys.exit(1)

            issuances = rpc.call('listissuances', asset_id)
            if not any(i['isreissuance'] and i['txid'] == txid and i['vin'] == vin for i in issuances):
                logging.error(f'Outpoint {txid}:{vin} in not associated with a reissuance.')
                sys.exit(1)

            reissuance_output = {'txid': txid, 'vin': vin}

        # register reissue on the Blockstream AMP platform
        details = rpc.call('gettransaction', txid).get('details')
        issuances = rpc.call('listissuances')
        listissuances = [issuance for issuance in issuances if issuance['txid'] == txid]

        confirm_payload = {'details': details, 'reissuance_output': reissuance_output, 'listissuances': listissuances}
        # TODO: write confirm payload to a file
        logging.info(f'calling "reissue-confirm" with payload: {confirm_payload}')

        confirm_url = base_url.format(f'assets/{asset_uuid}/reissue-confirm')
        response = requests.post(confirm_url, data=json.dumps(confirm_payload), headers=headers)

        if response.status_code != 200:
            logging.error(
                f'The transaction ({reissuance_output["txid"]}) has been broadcast, but the Blockstream AMP API "reissue-confirm" failed. '
                f'You will need to resend the payload again before do any kind of operations. '
                f'Run this script again with the additional argument '
                f'"--use-existing {reissuance_output["txid"]}:{reissuance_output["vin"]}". '
                f'Do not run this script again without the above extra argument as it will send the transaction again.')
            sys.exit(1)

        logging.info('Reissuance confirmed successfully')

    elif args.command == 'distribute':
        distribution_uuid = fj.get('distribution_uuid')
        if 'use_existing' not in args:
            # distribution specific checks
            check_assignments(base_url, headers, asset_uuid, distribution_uuid)

            # call the sendmany on the node and wait for confirmation
            map_address_amount = fj.get('map_address_amount')
            map_address_asset = fj.get('map_address_asset')
            txid = rpc.call('sendmany', '', map_address_amount, 0, '', [], False, 1, 'UNSET', map_address_asset)
            logging.info(f'Distribute transaction {txid}.')
            found = wait_for_confirmation(rpc, txid)
            if not found:
                sys.exit(1)
        else:  # confirm
            txid = args.use_existing

        # register distribution on the Blockstream AMP platform
        details = rpc.call('gettransaction', txid).get('details')
        tx_data = {'details': details, 'txid': txid}
        listunspent = rpc.call('listunspent')
        change_data = [u for u in listunspent if u['asset'] == asset_id and u['txid'] == txid]

        confirm_payload = {'tx_data': tx_data, 'change_data': change_data}
        # TODO: write confirm payload to a file
        logging.info(f'calling "distribution-confirm" with payload: {confirm_payload}')

        confirm_url = base_url.format(f'assets/{asset_uuid}/distributions/{distribution_uuid}/confirm')
        response = requests.post(confirm_url, data=json.dumps(confirm_payload), headers=headers)

        if response.status_code != 200:
            logging.error(
                f'The transaction ({txid}) has been broadcast, but the Blockstream AMP API "distribution-confirm" failed. '
                f'You will need to resend the payload again before do any kind of operations. '
                f'Run this script again with the additional argument '
                f'"--use-existing {txid}". '
                f'Do not run this script again without the above extra argument as it will send the transaction again.')
            sys.exit(1)

        logging.info('Distribution confirmed successfully')

    elif args.command == 'burn':
        if 'use_existing' not in args:
            amount = fj.get('amount')

            utxos = fj.get('utxos')
            check_utxos(rpc, utxos)

            local_amount = float(rpc.call('getbalance', '*', 0, False).get(asset_id, 0))
            if local_amount < amount:
                logging.error('local balance is lower than requested amount')
                sys.exit(1)

            txid = rpc.call('destroyamount', asset_id, amount)
            logging.info(f'Burn transaction {txid}.')
            found = wait_for_confirmation(rpc, txid)
            if not found:
                sys.exit(1)
        else:
            txid = args.use_existing

        # register distribution on the Blockstream AMP platform
        tx_data = {'txid': txid}
        listunspent = rpc.call('listunspent')
        change_data = [u for u in listunspent if u['asset'] == asset_id and u['txid'] == txid]

        confirm_payload = {'tx_data': tx_data, 'change_data': change_data}

        # TODO: write confirm payload to a file
        # we will have info about burn and about new change (if exists)
        logging.info(f'calling "burn-confirm" with payload: {confirm_payload}')

        confirm_url = base_url.format(f'assets/{asset_uuid}/burn-confirm')
        response = requests.post(confirm_url, data=json.dumps(confirm_payload), headers=headers)

        if response.status_code != 200:
            logging.error(
                f'The transaction ({txid}) has been broadcast, but the Blockstream AMP API "burn-confirm" failed. '
                f'You will need to resend the payload again before do any kind of operations. '
                f'Run this script again with the additional argument '
                f'"--use-existing {txid}". '
                f'Do not run this script again without the above extra argument as it will send the transaction again.')
            sys.exit(1)

        logging.info('Burn confirmed successfully')

    elif args.command == 'update-blinders':
        # get issuance outputs (1 or 2) with missing blinders via txs api
        txs_url = base_url.format(f'assets/{asset_uuid}/txs')
        response = requests.get(txs_url, headers=headers)

        if response.status_code != 200:
            logging.error('Cannot receive transaction list.')
            logging.error('The Blockstream AMP API "transaction list" failed. '
                          'Blinders will not be updated.')
            sys.exit(1)

        issuance = response.json()[0]
        vouts_to_update = {o['vout'] for o in issuance['outputs'] if o['asset_blinder'] == '00' * 32}
        issuance_details = rpc.call('gettransaction', issuance['txid'])['details']
        outputs_to_update = [{
            'txid': issuance['txid'],
            'vout': d['vout'],
            'asset_blinder': d['assetblinder'],
            'amount_blinder': d['amountblinder'],
        } for d in issuance_details if d['vout'] in vouts_to_update]

        if len(outputs_to_update) == 0:
            logging.error(
                'The blinders update list is empty.'
                'Blinders are present in AMP platform.')
            sys.exit(1)

        # send blinders to the platform via api
        confirm_url = base_url.format(f'assets/{asset_uuid}/update-blinders')
        for output in outputs_to_update:
            response = requests.post(confirm_url, data=json.dumps(output), headers=headers)
            if response.status_code != 200:
                logging.error(
                    'The blinders update list has been broadcast but the Blockstream AMP API "update blinders" failed.'
                    'You will need to resend the payload again. ')
                sys.exit(1)

        logging.info('update-blinders confirmed successfully')

    else:
        logging.error('Unimplemented command!')


if __name__ == '__main__':
    main()
