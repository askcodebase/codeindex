const os = require('os');

async function getTarget() {
    const arch = process.env.npm_config_arch || os.arch();

    switch (os.platform()) {
        case 'darwin':
            return arch === 'arm64' ? 'aarch64-apple-darwin' :
                'x86_64-apple-darwin';
        case 'win32':
            return arch === 'x64' ? 'x86_64-pc-windows-msvc' :
                arch === 'arm' ? 'aarch64-pc-windows-msvc' :
                    'i686-pc-windows-msvc';
        case 'linux':
            return arch === 'x64' ? 'x86_64-unknown-linux-musl' :
                arch === 'arm' ? 'arm-unknown-linux-gnueabihf' :
                    arch === 'armv7l' ? 'arm-unknown-linux-gnueabihf' :
                        arch === 'arm64' ? 'aarch64-unknown-linux-musl' :
                            arch === 'ppc64' ? 'powerpc64le-unknown-linux-gnu' :
                                arch === 's390x' ? 's390x-unknown-linux-gnu' :
                                    'i686-unknown-linux-musl'
        default: throw new Error('Unknown platform: ' + os.platform());
    }
}
console.log(getTarget())