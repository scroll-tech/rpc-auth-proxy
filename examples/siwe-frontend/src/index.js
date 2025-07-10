import { BrowserProvider, JsonRpcProvider, FetchRequest } from 'ethers';
import { SiweMessage } from 'siwe';
import { createWeb3Modal, defaultConfig } from '@web3modal/ethers';

const RPC_ENDPOINT = window.location.origin + '/api';

const projectId = 'd28384c92529a29af6c9537d45453308';
const mainnet = {
  chainId: 534352,
  name: 'Scroll Mainnet',
  currency: 'ETH',
  explorerUrl: 'https://scrollscan.com',
  rpcUrl: 'https://rpc.scroll.io',
};
const metadata = {
  name: 'My App',
  description: 'My App description',
  url: 'https://myapp.com',
  icons: ['https://avatars.myapp.com/']
};
const web3modal = createWeb3Modal({
  ethersConfig: defaultConfig({ metadata }),
  chains: [mainnet],
  projectId
});

let walletProvider = null;
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
    await web3modal.open();

    return new Promise((resolve, reject) => {
        const unsubscribe = web3modal.subscribeProvider((state) => {
            if (state.provider) {
                unsubscribe();

                try {
                    const provider = web3modal.getWalletProvider();
                    if (!provider) throw new Error('No wallet provider found');

                    walletProvider = new BrowserProvider(provider);

                    walletProvider.getSigner().then(signer => {
                        signer.getAddress().then(address => {
                            queryBalanceInput.value = address.toLowerCase();
                            resolve(address);
                        });
                    });
                } catch (err) {
                    reject(err);
                }
            }
        });

        setTimeout(() => {
            unsubscribe();
            reject(new Error('Connection timeout. Please try again.'));
        }, 120000);
    });
}

async function createMessage() {
    const signer = await walletProvider.getSigner();
    const address = await signer.getAddress();
    const nonce = await backendProvider.send('siwe_getNonce', []);
    const network = await backendProvider.getNetwork();

    const draft = new SiweMessage({
        domain: window.location.host,
        address: address,
        statement: 'Sign in with Ethereum to the app.',
        uri: RPC_ENDPOINT,
        version: '1',
        chainId: Number(network.chainId),
        nonce,
    });

    message = draft.prepareMessage();
    return `Message: ${message}`;
}

web3modal.subscribeProvider((state) => {
  if (!state.provider) {
    walletProvider = null;
    message = null;
    signature = null;

    document.getElementById('connectWalletLabel').innerText = '';
    document.getElementById('createMessageLabel').innerText = '';
    document.getElementById('signMessageLabel').innerText = '';
    document.getElementById('verifyLabel').innerText = '';
    document.getElementById('setTokenLabel').innerText = '';
    document.getElementById('queryBlockNumberLabel').innerText = '';
    document.getElementById('queryBalanceLabel').innerText = '';

    queryBalanceInput.value = '';
    setTokenInput.value = '';
  }
});

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

  if (jwt === '') throw new Error('Token not set');

  // re-create providers with bearer token
  const request = new FetchRequest(RPC_ENDPOINT);
  request.setHeader('Authorization', `Bearer ${jwt}`);
  backendProvider = new JsonRpcProvider(request);

  return `Token updated`;
}

async function queryBlockNumber() {
  // non-restricted methods like eth_chainId can be called without authentication
  const blockNumber = await backendProvider.getBlockNumber();
  return `Block number: ${blockNumber}`;
}

async function queryBalance() {
  // restricted methods like eth_getBalance can only be called with authentication
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
