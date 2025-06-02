import { BrowserProvider, JsonRpcProvider, FetchRequest } from 'ethers';
import { SiweMessage } from 'siwe';

const RPC_ENDPOINT = window.location.origin + '/api';

const walletProvider = new BrowserProvider(window.ethereum);
let backendProvider = new JsonRpcProvider(RPC_ENDPOINT);

const connectWalletBtn = document.getElementById('connectWalletBtn');
const createMessageBtn = document.getElementById('createMessageBtn');
const signMessageBtn = document.getElementById('signMessageBtn');
const verifyBtn = document.getElementById('verifyBtn');
const setTokenBtn = document.getElementById('setTokenBtn');
const queryBlockNumberBtn = document.getElementById('queryBlockNumberBtn');
const queryBalanceBtn = document.getElementById('queryBalanceBtn');

const queryBalanceInput = document.getElementById('queryBalanceInput');
const setTokenInput = document.getElementById('setTokenInput');

let message = null;
let signature = null;

async function connectWallet() {
    await walletProvider.send('eth_requestAccounts', []);
    const signer = await walletProvider.getSigner();
    queryBalanceInput.value = signer.address.toLowerCase();
    return signer.address;
}

async function createMessage() {
    const signer = await walletProvider.getSigner();
    const nonce = await backendProvider.send('siwe_getNonce', []);
    const network = await backendProvider.getNetwork();

    const draft = new SiweMessage({
        domain: window.location.host,
        address: signer.address,
        statement: 'Sign in with Ethereum to the app.',
        uri: RPC_ENDPOINT,
        version: '1',
        chainId: network.chainId,
        nonce,
    });

    message = draft.prepareMessage();
    return `Message: ${message}`;
}

async function signMessage() {
    const signer = await walletProvider.getSigner();
    signature = await signer.signMessage(message);
    return `Signature: ${signature}`;
}

async function sendForVerification() {
  const jwt = await backendProvider.send('siwe_signIn', [message, signature]);
  setTokenInput.value = jwt;
  return `Jwt: ${jwt}`;
}

async function setToken() {
  const jwt = setTokenInput.value;

  if (jwt === '') throw 'Token not set';

  // re-crate providers with bearer token
  const request = new FetchRequest(RPC_ENDPOINT);
  request.setHeader('Authorization', `Bearer ${jwt}`);
  backendProvider = new JsonRpcProvider(request);

  return `Token updated`;
}

async function queryBlockNumber() {
  // non-retricted methods like eth_chainId can be called without authentication
  const blockNumber = await backendProvider.getBlockNumber();
  return `Block number: ${blockNumber}`;
}

async function queryBalance() {
  // retricted methods like eth_getBalance can only be called with authentication
  const balance = await backendProvider.getBalance(queryBalanceInput.value);

  return `Balance: ${balance}`;
}

function wrapWithLabel(label, fn) {
  return async function () {
    try {
      const result = await fn();
      document.getElementById(label).innerText = `✅ ${result}`;
    } catch (err) {
      document.getElementById(label).innerText = `❌ ${err}`;
    }
  }
}

connectWalletBtn.onclick = wrapWithLabel('connectWalletLabel', connectWallet);
createMessageBtn.onclick = wrapWithLabel('createMessageLabel', createMessage);
signMessageBtn.onclick = wrapWithLabel('signMessageLabel', signMessage);
verifyBtn.onclick = wrapWithLabel('verifyLabel', sendForVerification);
setTokenBtn.onclick = wrapWithLabel('setTokenLabel', setToken);
queryBlockNumberBtn.onclick = wrapWithLabel('queryBlockNumberLabel', queryBlockNumber);
queryBalanceBtn.onclick = wrapWithLabel('queryBalanceLabel', queryBalance);
