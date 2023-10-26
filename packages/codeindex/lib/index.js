'use strict';

const path = require('path');

module.exports.codeindexPath = path.join(__dirname, `../bin/rg${process.platform === 'win32' ? '.exe' : ''}`);