// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util.ts";

test(function initString() {
	const init = "c=4&a=2&b=3&%C3%A1=1";
	const searchParams = new URLSearchParams(init);
	assert(init === searchParams.toString(), "The init query string does not match");
});

test(function initIterable() {
	const init = [["a", "54"], ["b", "true"]];
	const searchParams = new URLSearchParams(init);
	assertEqual(searchParams.toString(), "a=54&b=true");
});

test(function initRecord() {
	const init = { a: '54', b: 'true' };
	const searchParams = new URLSearchParams(init);
	assertEqual(searchParams.toString(), "a=54&b=true");
});

test(function appendSuccess() {
	const searchParams = new URLSearchParams();
	searchParams.append("a", "true");
	assertEqual(searchParams.toString(), "a=true");
});

test(function deleteSuccess() {
	const init = "a=54&b=true";
	const searchParams = new URLSearchParams(init);
	searchParams.delete("b");
	assertEqual(searchParams.toString(), "a=true");
});

test(function getAllSuccess() {
	const init = "a=54&b=true&a=true";
	const searchParams = new URLSearchParams(init);
	assertEqual(searchParams.getAll("a"), ["54", "true"]);
	assertEqual(searchParams.getAll("b"), ["true"]);
	assertEqual(searchParams.getAll("c"), []);
});

test(function getSuccess() {
	const init = "a=54&b=true&a=true";
	const searchParams = new URLSearchParams(init);
	assertEqual(searchParams.get("a"), "54");
	assertEqual(searchParams.get("b"), "true");
	assertEqual(searchParams.get("c"), null);
});

test(function hasSuccess() {
	const init = "a=54&b=true&a=true";
	const searchParams = new URLSearchParams(init);
	assert(searchParams.has("a"));
	assert(searchParams.has("b"));
	assert(!searchParams.has("c"));
});

test(function setSuccess() {
	const init = "a=54&b=true&a=true";
	const searchParams = new URLSearchParams(init);
	searchParams.set("a", "false");
	assertEqual(searchParams.toString(), "b=true&a=false");
});

test(function sortSuccess() {
	const init = "c=4&a=2&b=3&a=1";
	const searchParams = new URLSearchParams(init);
	searchParams.sort();
	assertEqual(searchParams.toString(), "a=2&a=1&b=3&c=4");
});

test(function forEachSuccess() {
	const init = [["a", "54"], ["b", "true"]];
	const searchParams = new URLSearchParams(init);
	let callNum = 0;
	searchParams.forEach((value, key, parent) => {
		assertEqual(searchParams, parent);
		assertEqual(value, init[callNum][1]);
		assertEqual(key, init[callNum][0]);
		callNum++;
	});
	assertEqual(callNum, init.length);
});

test(function missingName() {
	const init = "=4";
	const searchParams = new URLSearchParams(init);
	assertEqual(searchParams.get(""), "4");
	assertEqual(searchParams.toString(), "=4");
});

test(function missingValue() {
	const init = "4=";
	const searchParams = new URLSearchParams(init);
	assertEqual(searchParams.get("4"), "");
	assertEqual(searchParams.toString(), "4=");
});

test(function missingEqualSign() {
	const init = "4";
	const searchParams = new URLSearchParams(init);
	assertEqual(searchParams.get("4"), "");
	assertEqual(searchParams.toString(), "4=");
});

test(function missingPair() {
	const init = "c=4&&a=54&";
	const searchParams = new URLSearchParams(init);
	assertEqual(searchParams.toString(), "c=4&a=54");
});
