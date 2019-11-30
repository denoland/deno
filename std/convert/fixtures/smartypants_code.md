---
smartypants: true
description: SmartyPants does not modify characters within <pre>, <code>, <kbd>, or <script> tag blocks.
spec: https://daringfireball.net/projects/smartypants/
---
<pre>&amp;</pre>
<code>--foo</code>
<kbd>---foo</kbd>
<script>--foo</script>

Ensure that text such as custom tags that happen to
begin with the same letters as the above tags don't
match and thus benefit from Smartypants-ing.
<script-custom>--foo</script-custom>
`--foo` <codebar --foo codebar>
