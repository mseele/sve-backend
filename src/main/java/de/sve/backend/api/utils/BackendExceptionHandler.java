package de.sve.backend.api.utils;

import javax.ws.rs.core.Response;
import javax.ws.rs.core.Response.Status;
import javax.ws.rs.ext.ExceptionMapper;
import javax.ws.rs.ext.Provider;

@Provider
public class BackendExceptionHandler implements ExceptionMapper<BackendException> {

	@Override
	public Response toResponse(BackendException exception) {
		return Response.status(Status.BAD_REQUEST).entity(exception).build();
	}

}