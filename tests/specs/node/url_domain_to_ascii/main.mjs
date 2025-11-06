import url from 'node:url';

// Test valid domains
console.log('Valid domain:', url.domainToASCII('example.com'));
console.log('Valid unicode:', url.domainToASCII('münchen.de'));
console.log('Valid punycode:', url.domainToASCII('xn--mnchen-3ya.de'));

// Test invalid domains - should return empty string
console.log('Invalid punycode with unicode:', url.domainToASCII('xn--iñvalid.com'));
console.log('Invalid punycode prefix only:', url.domainToASCII('xn--'));
console.log('Empty string:', url.domainToASCII(''));
