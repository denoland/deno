Deno.test(
	{ perms: { net: true } },
	async function responseClone() {
		const response =
			await fetch(
				'http://localhost:4545/fixture.json',
			);
		const response1 =
			response.clone();
		assert(
			response !==
				response1,
		);
		assertEquals(
			response.status,
			response1
				.status,
		);
		assertEquals(
			response.statusText,
			response1
				.statusText,
		);
		const u8a =
			new Uint8Array(
				await response
					.arrayBuffer(),
			);
		const u8a1 =
			new Uint8Array(
				await response1
					.arrayBuffer(),
			);
		for (
			let i = 0;
			i <
				u8a.byteLength;
			i++
		) {
			assertEquals(
				u8a[i],
				u8a1[i],
			);
		}
	},
);
