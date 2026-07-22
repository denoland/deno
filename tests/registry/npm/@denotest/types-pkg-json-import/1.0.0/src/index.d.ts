export as namespace preact;

export interface VNode<P = {}> {
	type: any | string;
	props: P & { children: ComponentChildren };
	key: Key;
	/**
	 * ref is not guaranteed by React.ReactElement, for compatibility reasons
	 * with popular react libs we define it as optional too
	 */
	ref?: Ref<any> | null;
	/**
	 * The time this `vnode` started rendering. Will only be set when
	 * the devtools are attached.
	 * Default value: `0`
	 */
	startTime?: number;
	/**
	 * The time that the rendering of this `vnode` was completed. Will only be
	 * set when the devtools are attached.
	 * Default value: `-1`
	 */
	endTime?: number;
}

export type Key = string | number | any;

export type RefObject<T> = { current: T | null };
export type RefCallback<T> = (instance: T | null) => void;
export type Ref<T> = RefObject<T> | RefCallback<T> | null;

export type ComponentChild =
	| VNode<any>
	| object
	| string
	| number
	| bigint
	| boolean
	| null
	| undefined;
export type ComponentChildren = ComponentChild[] | ComponentChild;

export interface FunctionComponent<P = {}> {
	(props: any, context?: any): VNode<any> | null;
	displayName?: string;
	defaultProps?: Partial<P> | undefined;
}
export interface FunctionalComponent<P = {}> extends FunctionComponent<P> {}

//
// Context
// -----------------------------------
export interface Consumer<T>
	extends FunctionComponent<{
		children: (value: T) => ComponentChildren;
	}> {}
export interface PreactConsumer<T> extends Consumer<T> {}

export interface Provider<T>
	extends FunctionComponent<{
		value: T;
		children?: ComponentChildren;
	}> {}
export interface PreactProvider<T> extends Provider<T> {}
export type ContextType<C extends Context<any>> = C extends Context<infer T>
	? T
	: never;

export interface Context<T> {
	Consumer: Consumer<T>;
	Provider: Provider<T>;
	displayName?: string;
}
export interface PreactContext<T> extends Context<T> {}

export function createContext<T>(defaultValue: T): Context<T>;
