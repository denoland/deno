// @jsx preserve

let a = <></>;
a = <>foo</>;
a = <div />;
a = <div foo bar="baz" baz={1} foo-bar={fooBar} {...fooBar} />;
a = <div>foo</div>;
a = <Foo foo bar="baz" baz={1} foo-bar={fooBar} {...fooBar} />;
a = <Foo>foo</Foo>;
a = <Foo.Bar />;
a = <Foo.Bar>foo</Foo.Bar>;
