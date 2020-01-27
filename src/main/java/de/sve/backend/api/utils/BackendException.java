package de.sve.backend.api.utils;

public class BackendException extends Exception {

	public BackendException(String message, Throwable cause) {
		super(message, cause);
	}

	public BackendException(String message) {
		super(message);
	}
	
}
