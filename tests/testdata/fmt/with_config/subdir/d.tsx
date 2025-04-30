function Hello()
{
	// checks jsx.bracketPosition=sameLine
	// checks jsx.multiLineParens=never
	let a = <a
		href='https://example.com'
		target='_blank'>
		hi
	</a>

	return <div>
		{a}
		Hello
	</div>
}
