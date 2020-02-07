package de.sve.backend.api;

import java.util.logging.Level;
import java.util.logging.Logger;

import javax.ws.rs.POST;
import javax.ws.rs.Path;
import javax.ws.rs.Produces;
import javax.ws.rs.core.MediaType;

import de.sve.backend.api.utils.BackendException;
import de.sve.backend.manager.NewsManager;
import de.sve.backend.model.news.Subscription;

@Path("/news")
public class News {

	private static final Logger LOG = Logger.getLogger(News.class.getName());

	@Path("/subscribe")
	@POST
	@Produces(MediaType.APPLICATION_JSON)
	public void subscribe(Subscription subscription) throws BackendException {
		try {
			NewsManager.subscribe(subscription);
		} catch (Throwable t) {
			String message = "Error while subscribe to news"; //$NON-NLS-1$
			LOG.log(Level.SEVERE, message, t);
			throw new BackendException(message, t);
		}
	}

	@Path("/unsubscribe")
	@POST
	@Produces(MediaType.APPLICATION_JSON)
	public void unsubscribe(Subscription subscription) throws BackendException {
		try {
			NewsManager.unsubscribe(subscription);
		} catch (Throwable t) {
			String message = "Error while unsubscribe from news"; //$NON-NLS-1$
			LOG.log(Level.SEVERE, message, t);
			throw new BackendException(message, t);
		}
	}

}
