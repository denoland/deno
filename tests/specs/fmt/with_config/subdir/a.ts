Deno.test(
	{perms: {net: true}},
	async function responseClone()
	{
		const response =
			await fetch(
				'http://localhost:4545/assets/fixture.json'
			)
		const response1 =
			response.clone()
		assert(
			response
				!== response1
		)
		assertEquals(
			response.status,
			response1
				.status
		)
		assertEquals(
			response.statusText,
			response1
				.statusText
		)
		const u8a =
			await response
				.bytes()
		const u8a1 =
			await response1
				.bytes()
		for (
			let i = 0;
			i
				< u8a.byteLength;
			i++
		)
		{
			assertEquals(
				u8a[i],
				u8a1[i]
			)
		}

		// checks quoteProps=asNeeded
		let a = {
			'foo-bar': 1,
			foo: 2
		}

		let b
		// checks useBraces=maintain
		// checks spaceAround=true
		// checks singleBodyPosition=nextLine
		if ( true )
			b = 1

		// checks nextControlFlowPosition=nextLine
		do
		{
			console.log(
				'foo'
			)
		}
		while ( false )

		// checks operatorPosition=nextLine
		let c = 'hello world foo bar baz'
			+ 'hello world foo bar baz'
			+ 'hello world foo bar baz'

		// checks typeLiteral.separatorKind=comma
		// checks spaceSurroundingProperties=false
		type T = {a: 1, b: 2}
	}
)

// checks spaceSurroundingProperties=false
import {foo} from 'bar'
export {foo} from 'bar'
